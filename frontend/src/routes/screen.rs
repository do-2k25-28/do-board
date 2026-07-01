use dioxus::prelude::*;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::Deserialize;
use shared::{ClockConfig, ClockStyle, Screen as SharedScreen, SlideConfig, SlideTransition};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// ── Weather types ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone, PartialEq)]
struct WeatherData {
    location_name: String,
    current_temp: f64,
    current_feels_like: f64,
    current_weather_code: u16,
    wind_speed: f64,
    daily: Vec<DailyForecast>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct DailyForecast {
    date: String,
    temp_max: f64,
    temp_min: f64,
    weather_code: u16,
}

// ── Transport types ───────────────────────────────────────────────────────────

#[derive(Deserialize, Clone, PartialEq)]
struct DepartureInfo {
    line_code: String,
    line_color: String,
    text_color: String,
    direction: String,
    mode: String,
    wait_minutes: i64,
    time: String,
    realtime: bool,
}

#[derive(Deserialize, Clone, PartialEq)]
struct DeparturesResponse {
    departures: Vec<DepartureInfo>,
}

fn wmo_emoji(code: u16) -> &'static str {
    match code {
        0 => "☀️",
        1 => "🌤️",
        2 => "⛅",
        3 => "☁️",
        45 | 48 => "🌫️",
        51 | 53 | 55 => "🌦️",
        56 | 57 => "🌧️",
        61 | 63 | 65 => "🌧️",
        66 | 67 => "🌧️",
        71 | 73 | 75 => "❄️",
        77 => "🌨️",
        80..=82 => "🌦️",
        85 | 86 => "🌨️",
        95 => "⛈️",
        96 | 99 => "⛈️",
        _ => "🌡️",
    }
}

fn wmo_label(code: u16) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "-",
    }
}

fn day_label_from_date(date_str: &str) -> String {
    let js = format!(
        "(function(){{try{{var d=new Date('{date_str}T12:00:00');var s=d.toLocaleDateString('en-GB',{{weekday:'short'}});return s.charAt(0).toUpperCase()+s.slice(1);}}catch(e){{return '{}';}}}})();",
        &date_str[5..]
    );
    js_sys::eval(&js)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| date_str[5..].replace('-', "/"))
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Extract the video ID from a YouTube watch/share/embed/shorts URL, or treat
/// the input as a bare video ID if it doesn't look like a URL.
fn youtube_video_id(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let extract_after = |marker: &str| -> Option<String> {
        let idx = s.find(marker)? + marker.len();
        let rest = &s[idx..];
        let end = rest.find(['&', '?', '#']).unwrap_or(rest.len());
        let id = &rest[..end];
        (!id.is_empty()).then(|| id.to_string())
    };
    extract_after("v=")
        .or_else(|| extract_after("youtu.be/"))
        .or_else(|| extract_after("/embed/"))
        .or_else(|| extract_after("/shorts/"))
        .or_else(|| (!s.contains('/') && !s.contains('.')).then(|| s.to_string()))
}

/// Convert `https://host/path` → `https/host/path` (trailing slash if no path).
fn url_to_proxy_path(url: &str) -> String {
    let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        ("https", stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        ("http", stripped)
    } else {
        return url.to_string();
    };
    if rest.contains('/') {
        format!("{scheme}/{rest}")
    } else {
        format!("{scheme}/{rest}/")
    }
}

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

// Can be overridden independently of the HTTP API base at build time. When
// unset, derive a same-origin URL at runtime so the page's own reverse proxy
// (nginx forwarding /ws to the backend) is used instead of a hardcoded host.
fn ws_url() -> String {
    if let Some(v) = option_env!("WS_URL") {
        return v.to_string();
    }

    let location = web_sys::window().expect("no window").location();
    let scheme = if location.protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = location.host().unwrap_or_default();
    format!("{scheme}//{host}/ws")
}

// animation-fill-mode:backwards ensures the `from` keyframe is applied immediately
// on insertion, preventing a one-frame flash at the natural (untransformed) position.
const SLIDE_CSS: &str = r#"
@keyframes do-fade{from{opacity:0}to{opacity:1}}
@keyframes do-sl{from{transform:translateX(100%)}to{transform:translateX(0)}}
@keyframes do-sr{from{transform:translateX(-100%)}to{transform:translateX(0)}}
@keyframes do-su{from{transform:translateY(100%)}to{transform:translateY(0)}}
@keyframes do-sd{from{transform:translateY(-100%)}to{transform:translateY(0)}}
@keyframes do-zoom{from{transform:scale(.92);opacity:0}to{transform:scale(1);opacity:1}}
.tr-fade{animation:do-fade var(--tr-dur,500ms) ease-out both}
.tr-sl{animation:do-sl var(--tr-dur,500ms) cubic-bezier(.25,.46,.45,.94) both}
.tr-sr{animation:do-sr var(--tr-dur,500ms) cubic-bezier(.25,.46,.45,.94) both}
.tr-su{animation:do-su var(--tr-dur,500ms) cubic-bezier(.25,.46,.45,.94) both}
.tr-sd{animation:do-sd var(--tr-dur,500ms) cubic-bezier(.25,.46,.45,.94) both}
.tr-zoom{animation:do-zoom var(--tr-dur,500ms) ease-out both}
"#;

fn transition_class(t: &SlideTransition) -> &'static str {
    match t {
        SlideTransition::None => "",
        SlideTransition::Fade => "tr-fade",
        SlideTransition::SlideLeft => "tr-sl",
        SlideTransition::SlideRight => "tr-sr",
        SlideTransition::SlideUp => "tr-su",
        SlideTransition::SlideDown => "tr-sd",
        SlideTransition::Zoom => "tr-zoom",
    }
}

fn time_in_tz(timezone: &str) -> String {
    let code = format!(
        "new Date().toLocaleTimeString('fr-FR',\
         {{timeZone:'{}',hour12:false,hour:'2-digit',minute:'2-digit',second:'2-digit'}})",
        timezone
    );
    js_sys::eval(&code)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "--:--:--".to_string())
}

fn date_in_tz(timezone: &str) -> String {
    let code = format!(
        "new Date().toLocaleDateString('en-GB',\
         {{timeZone:'{}',weekday:'long',day:'numeric',month:'long',year:'numeric'}})",
        timezone
    );
    js_sys::eval(&code)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

fn time_parts_in_tz(timezone: &str) -> (u32, u32, u32) {
    let s = time_in_tz(timezone);
    let mut it = s.splitn(3, ':');
    let h = it.next().and_then(|p| p.trim().parse().ok()).unwrap_or(0);
    let m = it.next().and_then(|p| p.trim().parse().ok()).unwrap_or(0);
    let s = it.next().and_then(|p| p.trim().parse().ok()).unwrap_or(0);
    (h, m, s)
}

// ── Clock widget renderers ─────────────────────────────────────────────────────

fn render_digital_clock(clock: &ClockConfig) -> Element {
    let time = time_in_tz(&clock.timezone);
    let date = date_in_tz(&clock.timezone);
    let hm = time.get(..5).unwrap_or("--:--");
    let ss = time.get(6..8).unwrap_or("--");
    rsx! {
        div { class: "text-center select-none",
            if let Some(lbl) = &clock.label {
                p { class: "text-white/40 text-xs uppercase tracking-widest mb-4", "{lbl}" }
            }
            div { class: "font-mono tabular-nums flex items-start justify-center leading-none",
                span {
                    class: "text-white font-thin",
                    style: "font-size: clamp(3rem, 11vmin, 8rem);",
                    "{hm}"
                }
                span {
                    class: "text-white/40 font-thin",
                    style: "font-size: clamp(1.2rem, 3.5vmin, 2.8rem); margin-top: 0.45em;",
                    ":{ss}"
                }
            }
            p { class: "text-white/50 text-base mt-3", "{date}" }
            p { class: "text-white/20 text-xs mt-1 tracking-wider", "{clock.timezone}" }
        }
    }
}

fn render_analog_clock(clock: &ClockConfig) -> Element {
    let (h, m, s) = time_parts_in_tz(&clock.timezone);
    let pi = std::f64::consts::PI;

    // Angle from 12 o'clock, clockwise (degrees)
    let theta_s = (s as f64 / 60.0) * 360.0;
    let theta_m = ((m as f64 + s as f64 / 60.0) / 60.0) * 360.0;
    let theta_h = ((h as f64 % 12.0 + m as f64 / 60.0) / 12.0) * 360.0;

    // In SVG: tip_x = cx + r·sin(θ), tip_y = cy − r·cos(θ)
    let tip = |deg: f64, r: f64| -> (f64, f64) {
        let rad = deg * pi / 180.0;
        (100.0 + r * rad.sin(), 100.0 - r * rad.cos())
    };

    let (hx, hy) = tip(theta_h, 48.0);
    let (mx, my) = tip(theta_m, 65.0);
    let (sx, sy) = tip(theta_s, 78.0);
    let (stx, sty) = tip(theta_s + 180.0, 14.0); // second-hand tail

    struct Tick {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        major: bool,
    }
    let ticks: Vec<Tick> = (0..12u32)
        .map(|i| {
            let rad = i as f64 * 30.0 * pi / 180.0;
            let r_in = if i % 3 == 0 { 76.0 } else { 84.0 };
            Tick {
                x1: 100.0 + r_in * rad.sin(),
                y1: 100.0 - r_in * rad.cos(),
                x2: 100.0 + 92.0 * rad.sin(),
                y2: 100.0 - 92.0 * rad.cos(),
                major: i % 3 == 0,
            }
        })
        .collect();

    rsx! {
        div { class: "text-center select-none",
            if let Some(lbl) = &clock.label {
                p { class: "text-white/40 text-xs uppercase tracking-widest mb-3", "{lbl}" }
            }
            div { style: "width: 30vmin; height: 30vmin; margin: 0 auto;",
                svg {
                    "viewBox": "0 0 200 200",
                    width: "100%",
                    height: "100%",
                    // Outer ring
                    circle {
                        cx: "100", cy: "100", r: "96",
                        fill: "none",
                        stroke: "rgba(255,255,255,0.15)",
                        stroke_width: "1.5",
                    }
                    // Hour tick marks
                    for tick in ticks.iter() {
                        line {
                            x1: "{tick.x1:.2}", y1: "{tick.y1:.2}",
                            x2: "{tick.x2:.2}", y2: "{tick.y2:.2}",
                            stroke: "white",
                            stroke_width: if tick.major { "2.5" } else { "1" },
                            stroke_linecap: "round",
                            opacity: if tick.major { "0.65" } else { "0.22" },
                        }
                    }
                    // Hour hand
                    line {
                        x1: "100", y1: "100",
                        x2: "{hx:.2}", y2: "{hy:.2}",
                        stroke: "white",
                        stroke_width: "5",
                        stroke_linecap: "round",
                        opacity: "0.92",
                    }
                    // Minute hand
                    line {
                        x1: "100", y1: "100",
                        x2: "{mx:.2}", y2: "{my:.2}",
                        stroke: "white",
                        stroke_width: "2.5",
                        stroke_linecap: "round",
                        opacity: "0.85",
                    }
                    // Second hand + tail
                    line {
                        x1: "{stx:.2}", y1: "{sty:.2}",
                        x2: "{sx:.2}", y2: "{sy:.2}",
                        stroke: "#f87171",
                        stroke_width: "1.5",
                        stroke_linecap: "round",
                    }
                    // Center cap
                    circle { cx: "100", cy: "100", r: "5", fill: "#f87171" }
                    circle { cx: "100", cy: "100", r: "2.5", fill: "white" }
                }
            }
            p { class: "text-white/50 text-base mt-3", "{date_in_tz(&clock.timezone)}" }
            p { class: "text-white/20 text-xs mt-1 tracking-wider", "{clock.timezone}" }
        }
    }
}

#[component]
pub fn Screen() -> Element {
    let mut current_screen: Signal<Option<SharedScreen>> = use_signal(|| None);
    let mut current_slide: Signal<usize> = use_signal(|| 0);
    let mut clock_tick = use_signal(|| 0u32);
    let mut transition_key = use_signal(|| 0u32);
    let mut weather_cache: Signal<HashMap<String, WeatherData>> = use_signal(HashMap::new);

    // Hold the WebSocket alive for the lifetime of the component. Bumping
    // `reconnect_tick` tears down and re-establishes it.
    let mut ws: Signal<Option<web_sys::WebSocket>> = use_signal(|| None);
    let mut reconnect_tick = use_signal(|| 0u32);

    // Fetch default screen on mount
    use_effect(move || {
        spawn(async move {
            if let Ok(resp) = Request::get(&format!("{API_BASE}/api/screens/default"))
                .send()
                .await
            {
                if let Ok(Some(screen)) = resp.json::<Option<SharedScreen>>().await {
                    current_screen.set(Some(screen));
                    current_slide.set(0);
                    transition_key.set(transition_key() + 1);
                }
            }
        });
    });

    // Pre-fetch weather data for all weather slides whenever the screen changes
    use_effect(move || {
        let screen = current_screen.read().clone();
        if let Some(s) = screen {
            for slide in &s.slides {
                if let SlideConfig::Weather { location, days } = &slide.config {
                    let cache_key = format!("{}:{}", location, days);
                    if weather_cache.read().contains_key(&cache_key) {
                        continue;
                    }
                    let loc = location.clone();
                    let d = *days;
                    spawn(async move {
                        let encoded = percent_encode(&loc);
                        let url = format!("{API_BASE}/api/weather?location={encoded}&days={d}");
                        if let Ok(resp) = Request::get(&url).send().await {
                            if let Ok(wd) = resp.json::<WeatherData>().await {
                                weather_cache.write().insert(cache_key, wd);
                            }
                        }
                    });
                }
            }
        }
    });

    // WebSocket: receive pushed screens from backend. Re-runs (creating a
    // fresh connection) whenever `reconnect_tick` is bumped.
    use_effect(move || {
        let _ = reconnect_tick();

        let Ok(socket) = web_sys::WebSocket::new(&ws_url()) else {
            return;
        };

        let onmessage_cb = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
            if let Some(text) = event.data().as_string() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val.get("type").and_then(|t| t.as_str()) == Some("set_screen") {
                        if let Some(sv) = val.get("screen") {
                            if let Ok(screen) = serde_json::from_value::<SharedScreen>(sv.clone()) {
                                current_screen.set(Some(screen));
                                current_slide.set(0);
                                transition_key.set(transition_key() + 1);
                            }
                        }
                    }
                }
            }
        });
        socket.set_onmessage(Some(onmessage_cb.as_ref().unchecked_ref()));
        onmessage_cb.forget();

        // Idle-timeout intermediaries (reverse proxies, load balancers,
        // conntrack...) can silently drop a quiet WebSocket. When that
        // happens the backend marks the device offline, but without this the
        // page itself never notices - slides keep playing locally from
        // cached state while the dashboard shows the TV as gone. Reconnect
        // after a short delay instead.
        let onclose_cb = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::CloseEvent| {
            spawn(async move {
                TimeoutFuture::new(3_000).await;
                reconnect_tick.set(reconnect_tick() + 1);
            });
        });
        socket.set_onclose(Some(onclose_cb.as_ref().unchecked_ref()));
        onclose_cb.forget();

        // Heartbeat: send a no-op message often enough to keep any
        // intermediary's idle timeout from ever triggering, and to keep the
        // backend's `last_seen` fresh.
        let heartbeat_socket = socket.clone();
        spawn(async move {
            loop {
                TimeoutFuture::new(20_000).await;
                if heartbeat_socket.ready_state() != web_sys::WebSocket::OPEN {
                    break;
                }
                let _ = heartbeat_socket.send_with_str("ping");
            }
        });

        ws.set(Some(socket));
    });

    // Slide timer.
    // We record whether a screen was already loaded at the START of each wait
    // so that an initial no-screen poll (1 s) never advances the slide even if
    // the screen arrives during that second.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            let (duration, screen_was_loaded) = {
                let screen = current_screen.read();
                let loaded = screen.is_some();
                let dur = screen
                    .as_ref()
                    .and_then(|s| s.slides.get(current_slide()))
                    .map(|sl| sl.duration_secs * 1000)
                    .unwrap_or(1000);
                (dur, loaded)
            };
            TimeoutFuture::new(duration).await;
            if !screen_was_loaded {
                continue;
            }
            let total = current_screen
                .read()
                .as_ref()
                .map(|s| s.slides.len())
                .unwrap_or(0);
            if total > 1 {
                current_slide.set((current_slide() + 1) % total);
                transition_key.set(transition_key() + 1);
            }
        }
    });

    // Clock ticker (every second)
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            TimeoutFuture::new(1000).await;
            clock_tick.set(clock_tick() + 1);
        }
    });

    let total_slides = current_screen
        .read()
        .as_ref()
        .map(|s| s.slides.len())
        .unwrap_or(0);
    let idx = current_slide();
    let _tkey = transition_key();

    // Subscribe to clock_tick only for clock slides - avoids needless re-renders
    // (and potential iframe flicker) when the current slide is an iframe or other type.
    {
        let screen = current_screen.read();
        let is_clock = screen
            .as_ref()
            .and_then(|s| s.slides.get(idx))
            .map(|sl| matches!(sl.config, SlideConfig::Clock { .. }))
            .unwrap_or(false);
        if is_clock {
            let _ = clock_tick();
        }
    }

    let (anim_class, anim_dur_ms): (&'static str, u32) = {
        let screen = current_screen.read();
        let slide = screen.as_ref().and_then(|s| s.slides.get(idx));
        let cls = slide
            .map(|sl| transition_class(&sl.transition))
            .unwrap_or("");
        let dur = slide.map(|sl| sl.transition_duration_ms).unwrap_or(500);
        (cls, dur)
    };

    // Restart the CSS animation on every slide change.
    // We can't rely on `key` for this because Dioxus only guarantees remounts
    // for keyed list items, not single elements. Instead we remove the class,
    // force a synchronous reflow (offsetWidth), then re-add it.
    use_effect(move || {
        let _k = transition_key(); // reactive: re-run whenever transition_key changes
        let _ = js_sys::eval(
            r#"(function(){
                var el = document.getElementById('do-slide-anim');
                if (!el) return;
                var cls = Array.from(el.classList).find(c => c.startsWith('tr-'));
                if (!cls) return;
                el.classList.remove(cls);
                void el.offsetWidth; // force synchronous reflow to commit removal
                el.classList.add(cls);
            })()"#,
        );
    });

    let slide_content: Element = {
        let screen = current_screen.read();
        match screen.as_ref().and_then(|s| s.slides.get(idx)) {
            None if screen.is_some() => rsx! {
                div { class: "flex items-center justify-center h-full",
                    p { class: "text-white/30 text-lg", "No slides configured" }
                }
            },
            None => rsx! {
                div { class: "flex items-center justify-center h-full",
                    div { class: "text-center select-none",
                        p { class: "text-white/20 text-3xl font-thin tracking-widest uppercase",
                            "DO Board"
                        }
                        p { class: "text-white/15 text-sm mt-3", "No screen assigned" }
                    }
                }
            },
            Some(slide) => match &slide.config {
                SlideConfig::Iframe {
                    url,
                    cookies,
                    local_storage,
                    scroll_y_percent,
                } => {
                    // Scrolling requires injecting a script into the document, which
                    // only works when the page is served from our own origin — so a
                    // non-zero scroll forces the proxy path even with no cookies/LS.
                    let src = if local_storage.is_empty()
                        && cookies.is_empty()
                        && *scroll_y_percent == 0
                    {
                        url.clone()
                    } else {
                        let ls_map: HashMap<&str, &str> = local_storage
                            .iter()
                            .map(|e| (e.key.as_str(), e.value.as_str()))
                            .collect();
                        let cookies_map: HashMap<&str, &str> = cookies
                            .iter()
                            .map(|e| (e.key.as_str(), e.value.as_str()))
                            .collect();
                        let ls_json = serde_json::to_string(&ls_map).unwrap_or_default();
                        let cookies_json = serde_json::to_string(&cookies_map).unwrap_or_default();
                        // Path-based proxy: /api/iframe-proxy/{scheme}/{host}/{path}
                        // Assets load from the same proxy path → same origin → no CORS/module issues
                        let proxy_path = url_to_proxy_path(url);
                        format!(
                            "{API_BASE}/api/iframe-proxy/{proxy_path}?ls={}&cookies={}&scroll_y={}",
                            percent_encode(&ls_json),
                            percent_encode(&cookies_json),
                            scroll_y_percent,
                        )
                    };
                    rsx! {
                        iframe { src: "{src}", class: "w-full h-full border-0" }
                    }
                }
                SlideConfig::Clock { clocks } => {
                    let clocks = clocks.clone();
                    rsx! {
                        div { class: "flex items-center justify-center h-full gap-16 flex-wrap p-8",
                            for clock in clocks.iter() {
                                match clock.style {
                                    ClockStyle::Digital => render_digital_clock(clock),
                                    ClockStyle::Analog  => render_analog_clock(clock),
                                }
                            }
                        }
                    }
                }
                SlideConfig::Weather { location, days } => {
                    let location = location.clone();
                    let days = *days;
                    rsx! {
                        WeatherSlide { location, days, cache: weather_cache }
                    }
                }
                SlideConfig::Transport {
                    stop_id,
                    stop_name,
                    extra_stop_ids,
                    ..
                } => {
                    let stop_ids = std::iter::once(stop_id.as_str())
                        .chain(extra_stop_ids.iter().map(|s| s.as_str()))
                        .collect::<Vec<_>>()
                        .join(",");
                    let stop_name = stop_name.clone();
                    rsx! {
                        TransportSlide { stop_ids, stop_name }
                    }
                }
                SlideConfig::Birthdays { entries } => {
                    let today = {
                        let d = js_sys::Date::new_0();
                        format!("{:02}-{:02}", d.get_date(), d.get_month() + 1)
                    };
                    let current_year = js_sys::Date::new_0().get_full_year();
                    let today_entries: Vec<_> = entries
                        .iter()
                        .filter(|e| e.date.get(..5) == Some(today.as_str()))
                        .collect();
                    rsx! {
                        div { class: "flex flex-col items-center justify-center h-full gap-10 p-8",
                            p { class: "text-white/40 text-xs uppercase tracking-widest",
                                "🎂 Today's Birthdays"
                            }
                            if today_entries.is_empty() {
                                p { class: "text-white/25 text-2xl font-thin", "No birthdays today" }
                            } else {
                                div { class: "flex flex-wrap justify-center gap-10",
                                    for entry in today_entries.iter() {
                                        {
                                            let age: Option<u32> = entry.date
                                                .split('-')
                                                .nth(2)
                                                .and_then(|y| y.parse().ok())
                                                .map(|y: u32| current_year.saturating_sub(y));
                                            rsx! {
                                                div { class: "text-center select-none",
                                                    p {
                                                        class: "text-white font-medium",
                                                        style: "font-size: clamp(1.5rem, 4vmin, 3rem);",
                                                        "{entry.name}"
                                                    }
                                                    if let Some(a) = age {
                                                        p { class: "text-white/50 text-lg mt-2",
                                                            "{a} years old today"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                SlideConfig::Image { url } => {
                    let src = format!("{API_BASE}{url}");
                    rsx! {
                        div { class: "flex items-center justify-center h-full w-full",
                            img { src: "{src}", class: "max-w-full max-h-full object-contain" }
                        }
                    }
                }
                SlideConfig::Video { url } => match youtube_video_id(url) {
                    Some(id) => {
                        let src = format!(
                            "https://www.youtube-nocookie.com/embed/{id}?autoplay=1&mute=1&loop=1&playlist={id}&controls=0&modestbranding=1&rel=0&iv_load_policy=3",
                        );
                        rsx! {
                            iframe {
                                src: "{src}",
                                class: "w-full h-full border-0",
                                "allow": "autoplay; encrypted-media",
                            }
                        }
                    }
                    None => rsx! {
                        div { class: "flex items-center justify-center h-full",
                            p { class: "text-white/30 text-lg", "Invalid YouTube URL" }
                        }
                    },
                },
            },
        }
    };

    rsx! {
        style { {SLIDE_CSS} }
        div { class: "fixed inset-0 bg-zinc-950 overflow-hidden",
            div {
                id: "do-slide-anim",
                class: "absolute inset-0 w-full h-full {anim_class}",
                style: "--tr-dur:{anim_dur_ms}ms",
                {slide_content}
            }

            if total_slides > 1 {
                div { class: "absolute bottom-4 left-1/2 -translate-x-1/2 flex gap-2 z-10",
                    for i in 0..total_slides {
                        span {
                            class: if i == idx {
                                "w-2 h-2 rounded-full bg-white transition-all"
                            } else {
                                "w-2 h-2 rounded-full bg-white/25 transition-all"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Weather slide component ───────────────────────────────────────────────────

#[component]
fn WeatherSlide(
    location: String,
    days: u8,
    cache: Signal<HashMap<String, WeatherData>>,
) -> Element {
    let mut has_error = use_signal(|| false);
    let cache_key = format!("{}:{}", location, days);
    let key_for_render = cache_key.clone();
    let loc_display = location.clone();

    // Periodically refresh the cache entry for this location.
    // On first render the cache is already populated by Screen's pre-fetch,
    // so no loading state is shown unless this is the very first slide.
    use_coroutine(move |_: UnboundedReceiver<()>| {
        let loc = location.clone();
        let key = cache_key.clone();
        async move {
            loop {
                let encoded = percent_encode(&loc);
                let url = format!("{API_BASE}/api/weather?location={encoded}&days={days}");
                match Request::get(&url).send().await {
                    Ok(resp) if resp.ok() => {
                        if let Ok(wd) = resp.json::<WeatherData>().await {
                            cache.write().insert(key.clone(), wd);
                            has_error.set(false);
                        } else {
                            has_error.set(true);
                        }
                    }
                    _ => has_error.set(true),
                }
                TimeoutFuture::new(600_000).await;
            }
        }
    });

    let data = cache.read().get(&key_for_render).cloned();

    match data {
        None if has_error() => rsx! {
            div { class: "flex items-center justify-center h-full",
                div { class: "text-center select-none",
                    p { class: "text-white/40 text-4xl mb-4", "⚠️" }
                    p { class: "text-white/40 text-base", "Weather unavailable" }
                    p { class: "text-white/20 text-sm mt-2", "{loc_display}" }
                }
            }
        },
        None => rsx! {
            div { class: "flex items-center justify-center h-full",
                p { class: "text-white/30 text-sm select-none", "Loading weather…" }
            }
        },
        Some(wd) => {
            let emoji = wmo_emoji(wd.current_weather_code);
            let label = wmo_label(wd.current_weather_code);
            let temp = wd.current_temp.round() as i32;
            let feels = wd.current_feels_like.round() as i32;
            let wind = wd.wind_speed.round() as i32;
            let show_daily = days > 1 && wd.daily.len() > 1;
            let daily = wd.daily.clone();
            let loc_name = wd.location_name.clone();

            rsx! {
                div { class: "flex flex-col items-center justify-center h-full gap-6 p-8 select-none",
                    p { class: "text-white/40 text-sm uppercase tracking-widest", "{loc_name}" }

                    div { class: "flex flex-col items-center gap-2",
                        span { style: "font-size: clamp(3rem, 7vmin, 5rem); line-height:1;", "{emoji}" }
                        div {
                            class: "text-white font-thin tabular-nums leading-none",
                            style: "font-size: clamp(4rem, 15vmin, 10rem);",
                            "{temp}°"
                        }
                        p { class: "text-white/60 text-xl", "{label}" }
                        p { class: "text-white/30 text-sm",
                            "Feels like {feels}°  ·  Wind {wind} km/h"
                        }
                    }

                    if show_daily {
                        div { class: "flex gap-3 flex-wrap justify-center",
                            for (i, day) in daily.iter().enumerate() {
                                div {
                                    class: if i == 0 {
                                        "flex flex-col items-center gap-1 rounded-xl bg-white/10 px-4 py-3 min-w-16"
                                    } else {
                                        "flex flex-col items-center gap-1 rounded-xl bg-white/5 px-4 py-3 min-w-16"
                                    },
                                    p { class: "text-white/40 text-xs",
                                        if i == 0 { "Today" } else { "{day_label_from_date(&day.date)}" }
                                    }
                                    span { style: "font-size: 1.5rem; line-height:1;", "{wmo_emoji(day.weather_code)}" }
                                    p { class: "text-white font-medium text-sm", "{day.temp_max.round() as i32}°" }
                                    p { class: "text-white/30 text-xs", "{day.temp_min.round() as i32}°" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Transport ──────────────────────────────────────────────────────────────────

fn wait_label(dep: &DepartureInfo) -> String {
    if dep.wait_minutes <= 0 {
        "Now".to_string()
    } else if dep.wait_minutes >= 60 {
        dep.time.clone()
    } else {
        format!("{} min", dep.wait_minutes)
    }
}

#[component]
fn TransportSlide(stop_ids: String, stop_name: String) -> Element {
    let mut departures: Signal<Vec<DepartureInfo>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut config_error = use_signal(|| false);

    use_coroutine(move |_: UnboundedReceiver<()>| {
        let sid = stop_ids.clone();
        async move {
            loop {
                let url = format!(
                    "{API_BASE}/api/transport/departures?stop_id={}",
                    percent_encode(&sid)
                );
                match Request::get(&url).send().await {
                    Ok(resp) if resp.status() == 503 => {
                        config_error.set(true);
                        loading.set(false);
                    }
                    Ok(resp) if resp.ok() => {
                        if let Ok(data) = resp.json::<DeparturesResponse>().await {
                            departures.set(data.departures);
                            config_error.set(false);
                        }
                        loading.set(false);
                    }
                    _ => loading.set(false),
                }
                TimeoutFuture::new(30_000).await;
            }
        }
    });

    rsx! {
        div { class: "flex flex-col h-full p-8 select-none",
            // Header
            div { class: "flex items-center gap-4 mb-8",
                div { class: "flex-1 min-w-0",
                    p {
                        class: "text-white font-bold leading-tight truncate",
                        style: "font-size: clamp(1.6rem, 4vmin, 2.8rem);",
                        "{stop_name}"
                    }
                    p { class: "text-white/35 text-xs uppercase tracking-widest mt-1",
                        "TaM · Montpellier"
                    }
                }
                if loading() {
                    p { class: "text-white/25 text-sm animate-pulse shrink-0", "…" }
                }
            }

            if config_error() {
                div { class: "flex-1 flex flex-col items-center justify-center gap-3",
                    p { class: "text-white/30 text-4xl", "⚙️" }
                    p { class: "text-white/30 text-base text-center",
                        "Set "
                        code { class: "text-white/50", "GTFS_STATIC_URL" }
                        " and "
                        code { class: "text-white/50", "GTFS_RT_URL" }
                        " on the backend"
                    }
                }
            } else if departures.read().is_empty() && !loading() {
                div { class: "flex-1 flex items-center justify-center",
                    p { class: "text-white/25 text-xl", "No upcoming departures" }
                }
            } else {
                div { class: "flex flex-col divide-y divide-white/10",
                    for dep in departures.read().iter().take(8) {
                        {
                            let wlabel = wait_label(dep);
                            let badge_style = format!(
                                "background-color:#{};color:#{}",
                                dep.line_color, dep.text_color
                            );
                            let mode_icon = if dep.mode.contains("Tramway") { "🚃" }
                                           else { "🚌" };
                            let imminent = dep.wait_minutes <= 1;
                            let direction = dep.direction.clone();
                            let mode = dep.mode.clone();
                            let realtime = dep.realtime;
                            let line_code = dep.line_code.clone();
                            rsx! {
                                div { class: "flex items-center gap-4 py-4",
                                    div {
                                        class: "w-12 h-12 rounded-xl flex items-center justify-center font-bold shrink-0",
                                        style: "font-size: clamp(0.9rem, 2.5vmin, 1.3rem); {badge_style}",
                                        "{line_code}"
                                    }
                                    div { class: "flex-1 min-w-0",
                                        p {
                                            class: "text-white font-medium truncate",
                                            style: "font-size: clamp(1rem, 2.8vmin, 1.5rem);",
                                            "{direction}"
                                        }
                                        p { class: "text-white/35 text-xs mt-0.5",
                                            "{mode_icon} {mode}"
                                        }
                                    }
                                    div { class: "text-right shrink-0",
                                        p {
                                            class: if imminent {
                                                "text-yellow-300 font-bold"
                                            } else {
                                                "text-white font-semibold"
                                            },
                                            style: "font-size: clamp(1rem, 2.8vmin, 1.5rem);",
                                            "{wlabel}"
                                        }
                                        if realtime {
                                            p { class: "text-green-400/60 text-xs text-right mt-0.5",
                                                "real-time"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
