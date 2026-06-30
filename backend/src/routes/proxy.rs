use axum::{
    body::{to_bytes, Bytes},
    extract::{Path, Query, Request},
    http::{header, Method, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error as StdError;

#[derive(Deserialize, Default)]
pub struct ProxyQuery {
    #[serde(default)]
    pub ls: String,
    #[serde(default)]
    pub cookies: String,
}

/// Path-based reverse proxy: `/api/iframe-proxy/{scheme}/{host}/{*rest}`
///
/// - Fetches the target resource and serves it from our origin.
/// - For the HTML root: rewrites all root-relative and absolute URLs so that
///   subsequent asset/API requests go through this proxy, and injects the
///   localStorage/cookie script + a fetch/XHR patch.
/// - For all other resources: passes through as-is.
pub async fn proxy_all(
    Path(raw_path): Path<String>,
    Query(q): Query<ProxyQuery>,
    request: Request,
) -> impl IntoResponse {
    // Reconstruct target URL: "https/host/path" → "https://host/path"
    let base_url = if let Some(rest) = raw_path.strip_prefix("https/") {
        format!("https://{rest}")
    } else if let Some(rest) = raw_path.strip_prefix("http/") {
        format!("http://{rest}")
    } else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy path").into_response();
    };

    let scheme = if raw_path.starts_with("https/") {
        "https"
    } else {
        "http"
    };
    let after_scheme = &raw_path[scheme.len() + 1..]; // strip "https/"
    let host = after_scheme.split('/').next().unwrap_or("");
    let proxy_base = format!("/api/iframe-proxy/{scheme}/{host}");
    let target_origin = format!("{scheme}://{host}");

    // Extract method and headers before consuming the body
    let method = request.method().clone();

    let upstream_qs: String = request
        .uri()
        .query()
        .map(|qs| {
            qs.split('&')
                .filter(|p| !p.starts_with("ls=") && !p.starts_with("cookies="))
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default();
    let target_url = if upstream_qs.is_empty() {
        base_url
    } else {
        format!("{base_url}?{upstream_qs}")
    };

    // Collect all headers to forward (skip hop-by-hop headers)
    let fwd_headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .filter(|(n, _)| {
            !matches!(
                n.as_str(),
                "host"
                    | "connection"
                    | "transfer-encoding"
                    | "te"
                    | "trailer"
                    | "upgrade"
                    | "proxy-authorization"
                    | "proxy-connection"
                    | "keep-alive"
            )
        })
        .filter_map(|(n, v)| {
            v.to_str()
                .ok()
                .map(|s| (n.as_str().to_string(), s.to_string()))
        })
        .collect();

    let ls: HashMap<String, String> = serde_json::from_str(&q.ls).unwrap_or_default();
    let cookies_map: HashMap<String, String> = serde_json::from_str(&q.cookies).unwrap_or_default();

    // Consume the body (after all borrows of `request`)
    let body_bytes: Bytes = to_bytes(request.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap_or_default();

    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; DOBoard/1.0)")
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Client build error: {e}"),
            )
                .into_response()
        }
    };

    // Build upstream request with the correct method
    let mut upstream = match method {
        Method::POST => client.post(&target_url),
        Method::PUT => client.put(&target_url),
        Method::PATCH => client.patch(&target_url),
        Method::DELETE => client.delete(&target_url),
        Method::HEAD => client.head(&target_url),
        _ => client.get(&target_url),
    };

    // Forward all collected headers
    for (name, value) in &fwd_headers {
        upstream = upstream.header(name.as_str(), value.as_str());
    }

    // Forward body for methods that carry one
    if !body_bytes.is_empty() {
        upstream = upstream.body(body_bytes);
    }

    let resp = match upstream.send().await {
        Ok(r) => r,
        Err(e) => {
            let mut msg = format!("proxy error: {e}");
            let mut src: Option<&(dyn StdError + 'static)> = e.source();
            while let Some(s) = src {
                msg.push_str(&format!(" → {s}"));
                src = s.source();
            }
            return (StatusCode::BAD_GATEWAY, msg).into_response();
        }
    };

    let status = resp.status();

    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let is_html =
        content_type.contains("html") || content_type.is_empty() && target_url.ends_with('/');

    if !is_html {
        // Pass non-HTML content through as-is.
        // stale-while-revalidate lets the browser serve cached API responses
        // instantly while refreshing in the background, eliminating visual
        // flicker caused by brief "loading" states on every poll cycle.
        let bytes: Bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(_) => return (StatusCode::BAD_GATEWAY, "Failed to read body").into_response(),
        };
        let ct = if content_type.is_empty() {
            "application/octet-stream".to_string()
        } else {
            content_type
        };
        let cache_control = if method == Method::GET && status.is_success() {
            "max-age=5, stale-while-revalidate=60"
        } else {
            "no-store"
        };
        return (
            status,
            [
                (header::CONTENT_TYPE, ct),
                (header::CACHE_CONTROL, cache_control.to_string()),
            ],
            bytes,
        )
            .into_response();
    }

    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_GATEWAY, "Failed to read HTML body").into_response(),
    };

    let modified = rewrite_html(&body, &proxy_base, &target_origin, &ls, &cookies_map);

    (
        status,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        modified,
    )
        .into_response()
}

// ── HTML rewriting ────────────────────────────────────────────────────────────

fn rewrite_html(
    html: &str,
    proxy_base: &str,
    target_origin: &str,
    ls: &HashMap<String, String>,
    cookies_map: &HashMap<String, String>,
) -> String {
    // 1. Remove SRI integrity (hashes would fail since we serve from our origin)
    let html = remove_attr_with_value(html, "integrity");

    // 2. Remove crossorigin attribute (all forms)
    let html = html
        .replace(" crossorigin=\"anonymous\"", "")
        .replace(" crossorigin=\"use-credentials\"", "")
        .replace(" crossorigin=\"\"", "")
        .replace(" crossorigin", "");

    // 3. Rewrite root-relative paths in src/href attributes
    //    "/foo" → "{proxy_base}/foo"  (only rewrite if not already proxy URL)
    let html = rewrite_root_relative(&html, proxy_base);

    // 4. Rewrite absolute target-origin URLs remaining after step 3
    //    "https://host/path" → "{proxy_base}/path"  (double and single quotes)
    let html = html.replace(
        &format!("\"{}/", target_origin),
        &format!("\"{proxy_base}/"),
    );
    // bare origin replacement: keep the closing quote
    let html = html.replace(
        &format!("\"{}\"", target_origin),
        &format!("\"{proxy_base}/\""),
    );
    let html = html.replace(&format!("'{}/", target_origin), &format!("'{proxy_base}/"));
    let html = html.replace(&format!("'{}'", target_origin), &format!("'{proxy_base}/'"));

    // 5. Inject script (localStorage + cookies + fetch/XHR patch + disable SW)
    let injection = build_injection(ls, cookies_map, proxy_base, target_origin);
    inject_after_head(&html, &injection)
}

/// Replace `src="/...` and `href="/...` with `src="{proxy_base}/...`
/// Skips protocol-relative URLs (`//...`) and already-proxied URLs.
fn rewrite_root_relative(html: &str, proxy_base: &str) -> String {
    let already = format!("{proxy_base}/");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while !rest.is_empty() {
        // Find the next src=" or href=" occurrence
        let next = rest
            .find("src=\"/")
            .map(|i| (i, 6usize))
            .into_iter()
            .chain(rest.find("href=\"/").map(|i| (i, 7usize)))
            .min_by_key(|(i, _)| *i);

        let (pos, attr_len) = match next {
            Some(x) => x,
            None => {
                out.push_str(rest);
                break;
            }
        };

        let attr_str = &rest[pos..pos + attr_len]; // e.g. `src="/`
        let after_quote = &rest[pos + attr_len..]; // everything after the `"`/

        // Skip protocol-relative `//` and already-proxied paths
        if after_quote.starts_with('/') || after_quote.starts_with(&already[1..]) {
            out.push_str(&rest[..pos + attr_len]);
            rest = after_quote;
            continue;
        }

        // Rewrite
        let attr_name = &attr_str[..attr_len - 2]; // e.g. `src=`
        out.push_str(&rest[..pos]);
        out.push_str(&format!("{attr_name}\"{proxy_base}/"));
        rest = after_quote;
    }

    out
}

/// Remove `attr="value"` attributes (e.g. integrity="sha384-...").
fn remove_attr_with_value(html: &str, attr: &str) -> String {
    let search = format!(" {attr}=\"");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(&search) {
        out.push_str(&rest[..pos]);
        let after_open = &rest[pos + search.len()..];
        if let Some(close) = after_open.find('"') {
            rest = &after_open[close + 1..];
        } else {
            out.push_str(&rest[pos..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Inject `injection` right after the `<head` tag's closing `>`.
fn inject_after_head(html: &str, injection: &str) -> String {
    let lower = html.to_lowercase();
    let (tag, _tag_len) = if let Some(p) = lower.find("<head") {
        (p, 5)
    } else if let Some(p) = lower.find("<html") {
        (p, 5)
    } else {
        return format!("{injection}{html}");
    };
    if let Some(close) = html[tag..].find('>') {
        let insert = tag + close + 1;
        format!("{}{}{}", &html[..insert], injection, &html[insert..])
    } else {
        format!("{injection}{html}")
    }
}

// ── Injection script ──────────────────────────────────────────────────────────

fn build_injection(
    ls: &HashMap<String, String>,
    cookies_map: &HashMap<String, String>,
    proxy_base: &str,
    target_origin: &str,
) -> String {
    let pb_js = serde_json::to_string(proxy_base).unwrap_or_default();
    let to_js = serde_json::to_string(target_origin).unwrap_or_default();
    // <base href> tells SPA routers (Vue Router, etc.) where the app root is,
    // so they strip the proxy prefix from window.location.pathname when routing.
    let mut s =
        format!("<base href=\"{proxy_base}/\"><script>(function(){{var PB={pb_js};var TO={to_js};");

    // localStorage
    for (k, v) in ls {
        let k_js = serde_json::to_string(k).unwrap_or_default();
        let v_js = serde_json::to_string(v).unwrap_or_default();
        s.push_str(&format!(
            "try{{localStorage.setItem({k_js},{v_js})}}catch(e){{}};",
        ));
    }

    // cookies
    for (k, v) in cookies_map {
        let c =
            serde_json::to_string(&format!("{k}={v}; path=/; SameSite=Lax")).unwrap_or_default();
        s.push_str(&format!("try{{document.cookie={c}}}catch(e){{}};"));
    }

    // Fix URL and patch fetch/XHR so SPA routers see the logical path
    s.push_str(
        r#"function _rw(u){
  if(typeof u!=='string')return u;
  if(u.startsWith('/')&&!u.startsWith('//')&&!u.startsWith(PB))return PB+u;
  if(u===TO)return PB+'/';
  if(u.startsWith(TO+'/'))return PB+u.slice(TO.length);
  return u;
}
// Rewrite URL immediately so Vue Router / Nuxt see '/' instead of the proxy path.
// history.replaceState does not affect document.baseURI, so relative asset
// URLs already resolved during HTML parsing are unaffected.
try{
  if(location.pathname.startsWith(PB)){
    var _sp=location.pathname.slice(PB.length)||'/';
    history.replaceState(null,'',_sp+location.search+location.hash);
  }
}catch(e){}
var _f=window.fetch;
window.fetch=function(u,o){return _f.call(this,_rw(u),o);};
var _xo=XMLHttpRequest.prototype.open;
XMLHttpRequest.prototype.open=function(m,u){
  arguments[1]=_rw(u);
  _xo.apply(this,arguments);
};
try{
  var _la=location.assign.bind(location);
  location.assign=function(u){return _la(_rw(u));};
  var _lr=location.replace.bind(location);
  location.replace=function(u){return _lr(_rw(u));};
}catch(e){}
if(navigator.serviceWorker)try{
  navigator.serviceWorker.register=function(){return Promise.resolve({});};
}catch(e){}
})()</script>"#,
    );

    s
}
