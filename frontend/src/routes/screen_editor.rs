use crate::auth;
use crate::components::{Button, ButtonSize, ButtonVariant, Icon, Input, Label};
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_net::http::Request;
use js_sys;
use shared::{
    BirthdayEntry, ClockConfig, ClockStyle, KvEntry, Screen, Slide, SlideConfig, SlideTransition,
    TransportProvider, UpdateScreenRequest,
};
use uuid::Uuid;
use wasm_bindgen;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[derive(Clone, serde::Deserialize)]
struct StopLineResult {
    code: String,
    color: String,
    text_color: String,
    mode: String,
}

#[derive(Clone, serde::Deserialize)]
struct StopSearchResult {
    id: String,
    name: String,
    #[serde(default)]
    lines: Vec<StopLineResult>,
}

fn get_grouped_timezones() -> Vec<(String, Vec<String>)> {
    let flat: Vec<String> =
        js_sys::eval("try{Array.from(Intl.supportedValuesOf('timeZone'))}catch(e){null}")
            .ok()
            .filter(|v| !v.is_null())
            .map(|v| {
                js_sys::Array::from(&v)
                    .iter()
                    .filter_map(|x| x.as_string())
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![
                    "UTC".into(),
                    "Europe/Paris".into(),
                    "Europe/London".into(),
                    "America/New_York".into(),
                    "America/Los_Angeles".into(),
                    "Asia/Tokyo".into(),
                ]
            });
    let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for tz in flat {
        let region = tz.split('/').next().unwrap_or("Other").to_string();
        map.entry(region).or_default().push(tz);
    }
    map.into_iter().collect()
}

fn new_slide(slide_type: &str) -> Slide {
    let config = match slide_type {
        "weather" => SlideConfig::Weather {
            location: String::new(),
            days: 1,
        },
        "transport" => SlideConfig::Transport {
            provider: TransportProvider::Tam,
            stop_id: String::new(),
            stop_name: String::new(),
            extra_stop_ids: vec![],
        },
        "birthdays" => SlideConfig::Birthdays { entries: vec![] },
        "iframe" => SlideConfig::Iframe {
            url: String::new(),
            cookies: vec![],
            local_storage: vec![],
        },
        _ => SlideConfig::Clock {
            clocks: vec![ClockConfig {
                timezone: "Europe/Paris".into(),
                label: None,
                style: ClockStyle::Digital,
            }],
        },
    };
    Slide {
        id: Uuid::new_v4().to_string(),
        duration_secs: 10,
        config,
        transition: SlideTransition::Fade,
        transition_duration_ms: 500,
    }
}

fn transition_label(t: &SlideTransition) -> &'static str {
    match t {
        SlideTransition::None => "none",
        SlideTransition::Fade => "fade",
        SlideTransition::SlideLeft => "slide_left",
        SlideTransition::SlideRight => "slide_right",
        SlideTransition::SlideUp => "slide_up",
        SlideTransition::SlideDown => "slide_down",
        SlideTransition::Zoom => "zoom",
    }
}

fn label_to_transition(s: &str) -> SlideTransition {
    match s {
        "fade" => SlideTransition::Fade,
        "slide_left" => SlideTransition::SlideLeft,
        "slide_right" => SlideTransition::SlideRight,
        "slide_up" => SlideTransition::SlideUp,
        "slide_down" => SlideTransition::SlideDown,
        "zoom" => SlideTransition::Zoom,
        _ => SlideTransition::None,
    }
}

fn slide_label(config: &SlideConfig) -> &'static str {
    match config {
        SlideConfig::Weather { .. } => "Weather",
        SlideConfig::Transport { .. } => "Transport",
        SlideConfig::Birthdays { .. } => "Birthdays",
        SlideConfig::Iframe { .. } => "iFrame",
        SlideConfig::Clock { .. } => "Clock",
    }
}

fn slide_icon(config: &SlideConfig) -> &'static str {
    match config {
        SlideConfig::Weather { .. } => "cloud-sun",
        SlideConfig::Transport { .. } => "bus",
        SlideConfig::Birthdays { .. } => "cake",
        SlideConfig::Iframe { .. } => "globe",
        SlideConfig::Clock { .. } => "clock",
    }
}

#[component]
pub fn ScreenEditor(id: String) -> Element {
    let mut screen_name = use_signal(String::new);
    let mut slides: Signal<Vec<Slide>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut saving = use_signal(|| false);
    let mut save_error = use_signal(|| None::<String>);
    let mut editing_id: Signal<Option<String>> = use_signal(|| None);
    let mut adding_type: Signal<Option<String>> = use_signal(|| None);
    let screen_id = use_signal(|| id.clone());
    let nav = use_navigator();

    use_effect(move || {
        let sid = screen_id.read().clone();
        spawn(async move {
            let token = auth::get_token().unwrap_or_default();
            if let Ok(resp) = Request::get(&format!("{API_BASE}/api/screens/{sid}"))
                .header("Authorization", &format!("Bearer {token}"))
                .send()
                .await
            {
                if let Ok(screen) = resp.json::<Screen>().await {
                    screen_name.set(screen.name);
                    slides.set(screen.slides);
                }
            }
            loading.set(false);
        });
    });

    let do_save = move || {
        let sid = screen_id.read().clone();
        spawn(async move {
            saving.set(true);
            save_error.set(None);
            let token = auth::get_token().unwrap_or_default();
            let result = Request::put(&format!("{API_BASE}/api/screens/{sid}"))
                .header("Authorization", &format!("Bearer {token}"))
                .json(&UpdateScreenRequest {
                    name: screen_name(),
                    slides: slides.read().clone(),
                })
                .unwrap()
                .send()
                .await;
            saving.set(false);
            match result {
                Ok(r) if r.ok() => {}
                _ => save_error.set(Some("Failed to save. Please retry.".into())),
            }
        });
    };

    rsx! {
        div { class: "p-6 max-w-3xl mx-auto",
            // Header
            div { class: "flex items-center gap-3 mb-6",
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::IconSm,
                    onclick: move |_| { nav.push(Route::Screens {}); },
                    Icon { name: "arrow-left", size: "16" }
                }
                h1 { class: "text-2xl font-bold flex-1", "Screen Editor" }
                if let Some(err) = save_error() {
                    span { class: "text-sm text-destructive", "{err}" }
                }
                Button {
                    disabled: saving(),
                    onclick: move |_| do_save(),
                    if saving() { "Saving…" } else { "Save" }
                }
            }

            if loading() {
                div { class: "flex items-center justify-center p-16",
                    p { class: "text-muted-foreground text-sm", "Loading…" }
                }
            } else {
                // Screen name
                div { class: "flex flex-col gap-2 mb-6",
                    Label { html_for: "sname", "Screen name" }
                    Input {
                        id: "sname",
                        placeholder: "Living room…",
                        value: screen_name(),
                        oninput: move |v| screen_name.set(v),
                    }
                }

                // Slides list
                h2 { class: "font-semibold mb-3", "Slides ({slides.read().len()})" }

                div { class: "flex flex-col gap-2 mb-4",
                    for (idx, slide) in slides.read().iter().enumerate() {
                        SlideRow {
                            key: "{slide.id}",
                            slide: slide.clone(),
                            index: idx,
                            total: slides.read().len(),
                            is_editing: editing_id().as_deref() == Some(&slide.id),
                            on_edit: {
                                let sid = slide.id.clone();
                                move |_| {
                                    adding_type.set(None);
                                    if editing_id().as_deref() == Some(&sid) {
                                        editing_id.set(None);
                                    } else {
                                        editing_id.set(Some(sid.clone()));
                                    }
                                }
                            },
                            on_move_up: move |_| {
                                if idx == 0 { return; }
                                slides.write().swap(idx, idx - 1);
                            },
                            on_move_down: move |_| {
                                let len = slides.read().len();
                                if idx + 1 >= len { return; }
                                slides.write().swap(idx, idx + 1);
                            },
                            on_delete: move |_| {
                                slides.write().remove(idx);
                                editing_id.set(None);
                            },
                            on_save: move |updated: Slide| {
                                slides.write()[idx] = updated;
                                editing_id.set(None);
                            },
                        }
                    }
                }

                // Add slide panel
                if adding_type().is_none() {
                    button {
                        class: "w-full rounded-xl border-2 border-dashed border-border hover:border-ring py-4 flex items-center justify-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-colors",
                        onclick: move |_| {
                            editing_id.set(None);
                            adding_type.set(Some("weather".into()));
                        },
                        Icon { name: "plus", size: "16" }
                        "Add slide"
                    }
                } else {
                    div { class: "rounded-xl border bg-card p-4",
                        p { class: "text-sm font-medium mb-3", "Choose slide type" }
                        div { class: "grid grid-cols-3 sm:grid-cols-5 gap-2 mb-4",
                            for (key, label, icon) in [
                                ("weather",   "Weather",   "cloud-sun"),
                                ("transport", "Transport", "bus"),
                                ("birthdays", "Birthdays", "cake"),
                                ("iframe",    "iFrame",    "globe"),
                                ("clock",     "Clock",     "clock"),
                            ] {
                                button {
                                    class: if adding_type().as_deref() == Some(key) {
                                        "flex flex-col items-center gap-1 rounded-lg border-2 border-ring bg-accent p-3 text-xs font-medium transition-colors"
                                    } else {
                                        "flex flex-col items-center gap-1 rounded-lg border border-border p-3 text-xs font-medium hover:bg-accent transition-colors"
                                    },
                                    onclick: move |_| adding_type.set(Some(key.into())),
                                    Icon { name: icon, size: "20" }
                                    "{label}"
                                }
                            }
                        }
                        if let Some(stype) = adding_type() {
                            SlideForm {
                                slide: new_slide(&stype),
                                on_save: move |s: Slide| {
                                    slides.write().push(s);
                                    adding_type.set(None);
                                },
                                on_cancel: move |_| adding_type.set(None),
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Slide Row ────────────────────────────────────────────────────────────────

#[component]
fn SlideRow(
    slide: Slide,
    index: usize,
    total: usize,
    is_editing: bool,
    on_edit: EventHandler<()>,
    on_move_up: EventHandler<()>,
    on_move_down: EventHandler<()>,
    on_delete: EventHandler<()>,
    on_save: EventHandler<Slide>,
) -> Element {
    let icon = slide_icon(&slide.config);
    let label = slide_label(&slide.config);
    let duration = slide.duration_secs;
    let edit_icon: &'static str = if is_editing { "x" } else { "pencil" };
    let description: String = match &slide.config {
        SlideConfig::Weather { location, days } => {
            if *days > 1 {
                format!("{location} · {days}j")
            } else {
                location.clone()
            }
        }
        SlideConfig::Transport { stop_name, .. } => stop_name.clone(),
        SlideConfig::Birthdays { entries } => format!("{} entries", entries.len()),
        SlideConfig::Iframe { url, .. } => url.chars().take(30).collect::<String>(),
        SlideConfig::Clock { clocks } => clocks
            .iter()
            .map(|c| {
                c.label
                    .as_deref()
                    .unwrap_or(c.timezone.as_str())
                    .to_string()
            })
            .collect::<Vec<String>>()
            .join(", "),
    };

    rsx! {
        div { class: "rounded-xl border bg-card overflow-hidden",
            div { class: "flex items-center gap-3 px-4 py-3",
                span { class: "text-muted-foreground text-sm w-5 text-center shrink-0",
                    "{index + 1}"
                }
                Icon { name: icon, size: "16" }
                span { class: "flex-1 text-sm font-medium", "{label}" }
                span { class: "text-xs text-muted-foreground truncate max-w-32", "{description}" }
                span { class: "text-xs text-muted-foreground shrink-0", "{duration}s" }
                div { class: "flex items-center gap-1 ml-2 shrink-0",
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::IconXs,
                        disabled: index == 0,
                        onclick: move |_| on_move_up.call(()),
                        Icon { name: "chevron-up", size: "14" }
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::IconXs,
                        disabled: index + 1 >= total,
                        onclick: move |_| on_move_down.call(()),
                        Icon { name: "chevron-down", size: "14" }
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::IconXs,
                        onclick: move |_| on_edit.call(()),
                        Icon { name: edit_icon, size: "14" }
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::IconXs,
                        onclick: move |_| on_delete.call(()),
                        Icon { name: "trash-2", size: "14" }
                    }
                }
            }
            if is_editing {
                div { class: "border-t px-4 py-4",
                    SlideForm {
                        slide: slide.clone(),
                        on_save: move |s| on_save.call(s),
                        on_cancel: move |_| on_edit.call(()),
                    }
                }
            }
        }
    }
}

#[component]
fn SlideDescription(config: SlideConfig) -> Element {
    let text: String = match config {
        SlideConfig::Weather { location, days } => {
            if days > 1 {
                format!("{location} · {days}j")
            } else {
                location
            }
        }
        SlideConfig::Transport { stop_name, .. } => stop_name,
        SlideConfig::Birthdays { entries } => format!("{} entries", entries.len()),
        SlideConfig::Iframe { url, .. } => url.chars().take(30).collect::<String>(),
        SlideConfig::Clock { clocks } => clocks
            .iter()
            .map(|c| {
                c.label
                    .as_deref()
                    .unwrap_or(c.timezone.as_str())
                    .to_string()
            })
            .collect::<Vec<String>>()
            .join(", "),
    };
    rsx! {
        span { class: "text-xs text-muted-foreground truncate max-w-32", "{text}" }
    }
}

// ── Slide Form ───────────────────────────────────────────────────────────────

#[component]
fn SlideForm(slide: Slide, on_save: EventHandler<Slide>, on_cancel: EventHandler<()>) -> Element {
    let type_key: &'static str = match &slide.config {
        SlideConfig::Weather { .. } => "weather",
        SlideConfig::Transport { .. } => "transport",
        SlideConfig::Birthdays { .. } => "birthdays",
        SlideConfig::Iframe { .. } => "iframe",
        SlideConfig::Clock { .. } => "clock",
    };
    let slide_id = slide.id.clone();
    let mut duration = use_signal(|| slide.duration_secs);
    let init_transition = slide.transition.clone();
    let mut transition: Signal<SlideTransition> = use_signal(move || init_transition);
    let mut transition_dur = use_signal(|| slide.transition_duration_ms);

    // Weather
    let w_loc = if let SlideConfig::Weather { location, .. } = &slide.config {
        location.clone()
    } else {
        String::new()
    };
    let w_days: u8 = if let SlideConfig::Weather { days, .. } = &slide.config {
        *days
    } else {
        1
    };
    let mut weather_location = use_signal(move || w_loc);
    let mut weather_days = use_signal(move || w_days);

    // Transport
    let (t_stop_id, t_stop_name, t_extra) = if let SlideConfig::Transport {
        stop_id,
        stop_name,
        extra_stop_ids,
        ..
    } = &slide.config
    {
        (stop_id.clone(), stop_name.clone(), extra_stop_ids.clone())
    } else {
        (String::new(), String::new(), vec![])
    };
    let mut transport_stop_id = use_signal(move || t_stop_id);
    let mut transport_stop_name = use_signal(move || t_stop_name);
    let mut transport_extra_stop_ids: Signal<Vec<String>> = use_signal(move || t_extra);
    let mut transport_search_query = use_signal(String::new);
    let mut transport_search_results: Signal<Vec<StopSearchResult>> = use_signal(Vec::new);
    let mut transport_searching = use_signal(|| false);
    let mut transport_extra_search_query = use_signal(String::new);
    let mut transport_extra_search_results: Signal<Vec<StopSearchResult>> = use_signal(Vec::new);
    let mut transport_extra_searching = use_signal(|| false);

    // Birthdays
    let b_init = if let SlideConfig::Birthdays { entries } = &slide.config {
        entries.clone()
    } else {
        vec![]
    };
    let mut birthday_entries: Signal<Vec<BirthdayEntry>> = use_signal(move || b_init);
    let mut bday_new_name = use_signal(String::new);
    let mut bday_new_date_iso = use_signal(String::new); // yyyy-mm-dd from <input type="date">
    let mut bday_import_status = use_signal(|| Option::<String>::None);

    // iFrame
    let (i_url, i_cookies, i_local_storage) = if let SlideConfig::Iframe {
        url,
        cookies,
        local_storage,
    } = &slide.config
    {
        (url.clone(), cookies.clone(), local_storage.clone())
    } else {
        (String::new(), vec![], vec![])
    };
    let mut iframe_url = use_signal(move || i_url);
    let mut iframe_cookies: Signal<Vec<KvEntry>> = use_signal(move || i_cookies);
    let mut iframe_local_storage: Signal<Vec<KvEntry>> = use_signal(move || i_local_storage);
    let mut iframe_new_cookie_key = use_signal(String::new);
    let mut iframe_new_cookie_val = use_signal(String::new);
    let mut iframe_new_ls_key = use_signal(String::new);
    let mut iframe_new_ls_val = use_signal(String::new);

    // Clock
    let c_init = if let SlideConfig::Clock { clocks } = &slide.config {
        clocks.clone()
    } else {
        vec![ClockConfig {
            timezone: "Europe/Paris".into(),
            label: None,
            style: ClockStyle::Digital,
        }]
    };
    let mut clocks: Signal<Vec<ClockConfig>> = use_signal(move || c_init);
    let mut clock_new_tz = use_signal(|| "Europe/Paris".to_string());
    let mut clock_new_label = use_signal(String::new);
    let mut clock_new_style = use_signal(|| "digital".to_string());
    let tz_groups = use_signal(get_grouped_timezones);

    let mut do_save = move || {
        // Flush any pending iframe inputs before building the config
        if type_key == "iframe" {
            let ck = iframe_new_cookie_key();
            if !ck.trim().is_empty() {
                iframe_cookies.write().push(KvEntry {
                    key: ck,
                    value: iframe_new_cookie_val(),
                });
                iframe_new_cookie_key.set(String::new());
                iframe_new_cookie_val.set(String::new());
            }
            let lk = iframe_new_ls_key();
            if !lk.trim().is_empty() {
                iframe_local_storage.write().push(KvEntry {
                    key: lk,
                    value: iframe_new_ls_val(),
                });
                iframe_new_ls_key.set(String::new());
                iframe_new_ls_val.set(String::new());
            }
        }
        let config = match type_key {
            "weather" => SlideConfig::Weather {
                location: weather_location(),
                days: weather_days(),
            },
            "transport" => SlideConfig::Transport {
                provider: TransportProvider::Tam,
                stop_id: transport_stop_id(),
                stop_name: transport_stop_name(),
                extra_stop_ids: transport_extra_stop_ids(),
            },
            "birthdays" => SlideConfig::Birthdays {
                entries: birthday_entries(),
            },
            "iframe" => SlideConfig::Iframe {
                url: iframe_url(),
                cookies: iframe_cookies(),
                local_storage: iframe_local_storage(),
            },
            _ => SlideConfig::Clock { clocks: clocks() },
        };
        on_save.call(Slide {
            id: slide_id.clone(),
            duration_secs: duration(),
            config,
            transition: transition(),
            transition_duration_ms: transition_dur(),
        });
    };

    rsx! {
        div { class: "flex flex-col gap-4",
            // Duration
            div { class: "flex items-center gap-3",
                Label { html_for: "dur", "Duration (seconds)" }
                input {
                    id: "dur",
                    r#type: "number",
                    min: "1",
                    max: "300",
                    value: "{duration}",
                    class: "w-24 border-input flex h-9 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u32>() { duration.set(v); }
                    },
                }
            }

            // Transition
            div { class: "flex flex-col gap-2",
                Label { html_for: "trans", "Transition" }
                div { class: "flex gap-1.5 flex-wrap",
                    for (tkey, tlabel) in [
                        ("none",        "None"),
                        ("fade",        "Fade"),
                        ("slide_left",  "From right"),
                        ("slide_right", "From left"),
                        ("slide_up",    "From bottom"),
                        ("slide_down",  "From top"),
                        ("zoom",        "Zoom"),
                    ] {
                        button {
                            r#type: "button",
                            class: if transition_label(&transition()) == tkey {
                                "text-xs px-2.5 py-1 rounded-md border-2 border-ring bg-accent font-medium transition-colors"
                            } else {
                                "text-xs px-2.5 py-1 rounded-md border border-border hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                            },
                            onclick: move |_| transition.set(label_to_transition(tkey)),
                            "{tlabel}"
                        }
                    }
                }

            if transition() != SlideTransition::None {
                div { class: "flex flex-col gap-2 mt-1",
                    Label { "Speed" }
                    div { class: "flex gap-1.5 flex-wrap",
                        for (ms, label) in [
                            (150u32,  "Very fast"),
                            (300u32,  "Fast"),
                            (500u32,  "Normal"),
                            (800u32,  "Slow"),
                            (1500u32, "Very slow"),
                        ] {
                            button {
                                r#type: "button",
                                class: if transition_dur() == ms {
                                    "text-xs px-2.5 py-1 rounded-md border-2 border-ring bg-accent font-medium transition-colors"
                                } else {
                                    "text-xs px-2.5 py-1 rounded-md border border-border hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                                },
                                onclick: move |_| transition_dur.set(ms),
                                "{label}"
                            }
                        }
                    }
                }
            }
            }

            // Type-specific - Weather
            if type_key == "weather" {
                div { class: "flex flex-col gap-3",
                    div { class: "flex flex-col gap-2",
                        Label { html_for: "wloc", "Location" }
                        Input {
                            id: "wloc",
                            placeholder: "Montpellier, France",
                            value: weather_location(),
                            oninput: move |v| weather_location.set(v),
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        Label { "Days to display" }
                        div { class: "flex gap-1.5 flex-wrap",
                            for d in 1u8..=7u8 {
                                button {
                                    r#type: "button",
                                    class: if weather_days() == d {
                                        "text-xs px-2.5 py-1 rounded-md border-2 border-ring bg-accent font-medium transition-colors"
                                    } else {
                                        "text-xs px-2.5 py-1 rounded-md border border-border hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                                    },
                                    onclick: move |_| weather_days.set(d),
                                    if d == 1 { "Today" } else { "{d} days" }
                                }
                            }
                        }
                        p { class: "text-xs text-muted-foreground",
                            "API : "
                            a {
                                href: "https://open-meteo.com",
                                target: "_blank",
                                class: "underline",
                                "Open-Meteo"
                            }
                            " - free, no API key"
                        }
                    }
                }
            }

            // Type-specific - Transport
            if type_key == "transport" {
                div { class: "flex flex-col gap-3",
                    div { class: "rounded-md border bg-muted/30 px-3 py-2 text-xs text-muted-foreground",
                        "Provider: "
                        span { class: "font-medium text-foreground", "TaM · Montpellier" }
                        " - set "
                        code { class: "text-foreground/70", "GTFS_STATIC_URL" }
                        " and "
                        code { class: "text-foreground/70", "GTFS_RT_URL" }
                        " on the backend"
                    }

                    // Stop search
                    div { class: "flex flex-col gap-2",
                        Label { "Search stop" }
                        div { class: "flex gap-2",
                            input {
                                class: "flex h-9 flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                                placeholder: "Place de la Comédie…",
                                value: transport_search_query(),
                                oninput: move |e| transport_search_query.set(e.value()),
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter && !transport_searching() {
                                        let q = transport_search_query();
                                        if q.trim().is_empty() { return; }
                                        transport_searching.set(true);
                                        transport_search_results.set(vec![]);
                                        spawn(async move {
                                            let q_enc = js_sys::encode_uri_component(q.trim()).as_string().unwrap_or_default();
                                            let url = format!("{API_BASE}/api/transport/stops?q={q_enc}");
                                            if let Ok(resp) = Request::get(&url).send().await {
                                                if resp.ok() {
                                                    if let Ok(r) = resp.json::<Vec<StopSearchResult>>().await {
                                                        transport_search_results.set(r);
                                                    }
                                                }
                                            }
                                            transport_searching.set(false);
                                        });
                                    }
                                },
                            }
                            Button {
                                variant: ButtonVariant::Secondary,
                                size: ButtonSize::Sm,
                                disabled: transport_searching(),
                                onclick: move |_| {
                                    let q = transport_search_query();
                                    if q.trim().is_empty() { return; }
                                    transport_searching.set(true);
                                    transport_search_results.set(vec![]);
                                    spawn(async move {
                                        let q_enc = js_sys::encode_uri_component(q.trim()).as_string().unwrap_or_default();
                                        let url = format!("{API_BASE}/api/transport/stops?q={q_enc}");
                                        if let Ok(resp) = Request::get(&url).send().await {
                                            if resp.ok() {
                                                if let Ok(r) = resp.json::<Vec<StopSearchResult>>().await {
                                                    transport_search_results.set(r);
                                                }
                                            }
                                        }
                                        transport_searching.set(false);
                                    });
                                },
                                if transport_searching() { "…" } else { "Search" }
                            }
                        }
                    }

                    // Search results
                    if !transport_search_results.read().is_empty() {
                        div { class: "border rounded-md divide-y max-h-44 overflow-y-auto",
                            for stop in transport_search_results.read().clone().into_iter() {
                                {
                                    let id2 = stop.id.clone();
                                    let name2 = stop.name.clone();
                                    rsx! {
                                        button {
                                            r#type: "button",
                                            class: "w-full text-left px-3 py-2 text-sm hover:bg-accent transition-colors",
                                            onclick: move |_| {
                                                transport_stop_id.set(id2.clone());
                                                transport_stop_name.set(name2.clone());
                                                transport_search_results.set(vec![]);
                                            },
                                            div { class: "flex items-center gap-2",
                                                span { class: "font-medium truncate flex-1", "{stop.name}" }
                                                div { class: "flex gap-1 shrink-0",
                                                    for line in &stop.lines {
                                                        {
                                                            let style = format!("background-color:#{};color:#{}", line.color, line.text_color);
                                                            let code = line.code.clone();
                                                            rsx! {
                                                                span {
                                                                    class: "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-bold leading-none",
                                                                    style: "{style}",
                                                                    "{code}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            div { class: "text-muted-foreground text-xs", "{stop.id}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "flex flex-col gap-2",
                        Label { html_for: "tstopname", "Display name" }
                        Input {
                            id: "tstopname",
                            placeholder: "Place de la Comédie",
                            value: transport_stop_name(),
                            oninput: move |v| transport_stop_name.set(v),
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        Label { html_for: "tstopid", "Stop ID (GTFS)" }
                        Input {
                            id: "tstopid",
                            placeholder: "StopPoint:MFRA:SP:…",
                            value: transport_stop_id(),
                            oninput: move |v| transport_stop_id.set(v),
                        }
                    }

                    // Extra stops (other directions)
                    div { class: "flex flex-col gap-2 pt-1 border-t",
                        p { class: "text-sm font-medium", "Other direction(s)" }

                        if !transport_extra_stop_ids.read().is_empty() {
                            div { class: "flex flex-col gap-1",
                                for (i, sid) in transport_extra_stop_ids.read().iter().enumerate() {
                                    div { class: "flex items-center gap-2 rounded-md border px-3 py-1.5 text-sm",
                                        span { class: "flex-1 font-mono text-xs text-muted-foreground", "{sid}" }
                                        button {
                                            r#type: "button",
                                            class: "text-muted-foreground hover:text-destructive",
                                            onclick: move |_| { transport_extra_stop_ids.write().remove(i); },
                                            Icon { name: "x", size: "14" }
                                        }
                                    }
                                }
                            }
                        }

                        div { class: "flex gap-2",
                            input {
                                class: "flex h-9 flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                                placeholder: "Search stop for return direction…",
                                value: transport_extra_search_query(),
                                oninput: move |e| transport_extra_search_query.set(e.value()),
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter && !transport_extra_searching() {
                                        let q = transport_extra_search_query();
                                        if q.trim().is_empty() { return; }
                                        transport_extra_searching.set(true);
                                        transport_extra_search_results.set(vec![]);
                                        spawn(async move {
                                            let q_enc = js_sys::encode_uri_component(q.trim()).as_string().unwrap_or_default();
                                            let url = format!("{API_BASE}/api/transport/stops?q={q_enc}");
                                            if let Ok(resp) = Request::get(&url).send().await {
                                                if resp.ok() {
                                                    if let Ok(r) = resp.json::<Vec<StopSearchResult>>().await {
                                                        transport_extra_search_results.set(r);
                                                    }
                                                }
                                            }
                                            transport_extra_searching.set(false);
                                        });
                                    }
                                },
                            }
                            Button {
                                variant: ButtonVariant::Secondary,
                                size: ButtonSize::Sm,
                                disabled: transport_extra_searching(),
                                onclick: move |_| {
                                    let q = transport_extra_search_query();
                                    if q.trim().is_empty() { return; }
                                    transport_extra_searching.set(true);
                                    transport_extra_search_results.set(vec![]);
                                    spawn(async move {
                                        let q_enc = js_sys::encode_uri_component(q.trim()).as_string().unwrap_or_default();
                                        let url = format!("{API_BASE}/api/transport/stops?q={q_enc}");
                                        if let Ok(resp) = Request::get(&url).send().await {
                                            if resp.ok() {
                                                if let Ok(r) = resp.json::<Vec<StopSearchResult>>().await {
                                                    transport_extra_search_results.set(r);
                                                }
                                            }
                                        }
                                        transport_extra_searching.set(false);
                                    });
                                },
                                if transport_extra_searching() { "…" } else { "Search" }
                            }
                        }

                        if !transport_extra_search_results.read().is_empty() {
                            div { class: "border rounded-md divide-y max-h-36 overflow-y-auto",
                                for stop in transport_extra_search_results.read().clone().into_iter() {
                                    {
                                        let id2 = stop.id.clone();
                                        rsx! {
                                            button {
                                                r#type: "button",
                                                class: "w-full text-left px-3 py-2 text-sm hover:bg-accent transition-colors",
                                                onclick: move |_| {
                                                    let ids = transport_extra_stop_ids.read().clone();
                                                    if !ids.contains(&id2) {
                                                        transport_extra_stop_ids.write().push(id2.clone());
                                                    }
                                                    transport_extra_search_results.set(vec![]);
                                                    transport_extra_search_query.set(String::new());
                                                },
                                                div { class: "flex items-center gap-2",
                                                    span { class: "font-medium truncate flex-1", "{stop.name}" }
                                                    div { class: "flex gap-1 shrink-0",
                                                        for line in &stop.lines {
                                                            {
                                                                let style = format!("background-color:#{};color:#{}", line.color, line.text_color);
                                                                let code = line.code.clone();
                                                                rsx! {
                                                                    span {
                                                                        class: "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-bold leading-none",
                                                                        style: "{style}",
                                                                        "{code}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div { class: "text-muted-foreground text-xs", "{stop.id}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Type-specific - Birthdays
            if type_key == "birthdays" {
                div { class: "flex flex-col gap-4",

                    // Entry list
                    if !birthday_entries.read().is_empty() {
                        div { class: "flex flex-col divide-y rounded-lg border max-h-52 overflow-y-auto",
                            for (i, entry) in birthday_entries.read().iter().enumerate() {
                                div { class: "flex items-center gap-2 px-3 py-2 text-sm",
                                    span { class: "flex-1", "{entry.name}" }
                                    span { class: "font-mono text-xs text-muted-foreground", "{entry.date}" }
                                    button {
                                        class: "text-muted-foreground hover:text-destructive",
                                        onclick: move |_| { birthday_entries.write().remove(i); },
                                        Icon { name: "x", size: "14" }
                                    }
                                }
                            }
                        }
                    }

                    // Manual add
                    div { class: "flex gap-2 items-end flex-wrap",
                        div { class: "flex flex-col gap-1 flex-1 min-w-32",
                            Label { html_for: "bname", "Name" }
                            Input {
                                id: "bname",
                                placeholder: "Alice Martin",
                                value: bday_new_name(),
                                oninput: move |v| bday_new_name.set(v),
                            }
                        }
                        div { class: "flex flex-col gap-1",
                            Label { html_for: "bdate", "Date of birth" }
                            input {
                                id: "bdate",
                                r#type: "date",
                                class: "border-input flex h-9 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring",
                                value: bday_new_date_iso(),
                                oninput: move |e| bday_new_date_iso.set(e.value()),
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let name = bday_new_name().trim().to_string();
                                let iso = bday_new_date_iso(); // "yyyy-mm-dd"
                                if name.is_empty() || iso.is_empty() { return; }
                                // Convert yyyy-mm-dd → dd-mm-yyyy
                                let parts: Vec<&str> = iso.split('-').collect();
                                let date = if parts.len() == 3 {
                                    format!("{}-{}-{}", parts[2], parts[1], parts[0])
                                } else {
                                    iso
                                };
                                birthday_entries.write().push(BirthdayEntry { name, date });
                                bday_new_name.set(String::new());
                                bday_new_date_iso.set(String::new());
                            },
                            "Add"
                        }
                    }

                    // Excel import / template
                    div { class: "flex items-center gap-3 pt-1 border-t",
                        // File upload
                        label {
                            class: "cursor-pointer inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground border border-border rounded-md px-3 h-9 hover:bg-accent transition-colors",
                            Icon { name: "upload", size: "14" }
                            "Import Excel"
                            input {
                                r#type: "file",
                                accept: ".xlsx",
                                class: "hidden",
                                onchange: move |evt: Event<FormData>| async move {
                                    bday_import_status.set(Some("Importing…".to_string()));
                                    let files = evt.files();
                                    if let Some(file) = files.first() {
                                        let Ok(bytes) = file.read_bytes().await else { return; };
                                        let arr = js_sys::Uint8Array::from(bytes.as_ref());
                                        let body = wasm_bindgen::JsValue::from(arr);
                                        match Request::post(&format!("{API_BASE}/api/birthdays/import"))
                                            .header("content-type", "application/octet-stream")
                                            .body(body)
                                            .unwrap()
                                            .send()
                                            .await
                                        {
                                            Ok(resp) if resp.ok() => {
                                                if let Ok(imported) = resp.json::<Vec<BirthdayEntry>>().await {
                                                    let n = imported.len();
                                                    birthday_entries.write().extend(imported);
                                                    bday_import_status.set(Some(format!("{n} entries imported")));
                                                } else {
                                                    bday_import_status.set(Some("Parse error".to_string()));
                                                }
                                            }
                                            _ => bday_import_status.set(Some("Import failed".to_string())),
                                        }
                                    }
                                }
                            }
                        }

                        // Download template
                        a {
                            href: "{API_BASE}/api/birthdays/template",
                            download: "birthdays-template.xlsx",
                            class: "inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground",
                            Icon { name: "download", size: "14" }
                            "Download template"
                        }

                        if let Some(status) = bday_import_status() {
                            span { class: "text-xs text-muted-foreground ml-auto", "{status}" }
                        }
                    }
                }
            }

            // Type-specific - iFrame
            if type_key == "iframe" {
                div { class: "flex flex-col gap-4",
                    div { class: "flex flex-col gap-2",
                        Label { html_for: "iurl", "URL" }
                        Input {
                            id: "iurl",
                            placeholder: "https://example.com",
                            value: iframe_url(),
                            oninput: move |v| iframe_url.set(v),
                        }
                    }

                    // Cookies
                    div { class: "flex flex-col gap-2",
                        p { class: "text-sm font-medium", "Cookies" }
                        if !iframe_cookies.read().is_empty() {
                            div { class: "flex flex-col divide-y rounded-lg border",
                                for (i, entry) in iframe_cookies.read().iter().enumerate() {
                                    div { class: "flex items-center gap-2 px-3 py-2 text-sm",
                                        span { class: "font-mono text-xs text-muted-foreground w-32 truncate", "{entry.key}" }
                                        span { class: "flex-1 text-xs truncate", "{entry.value}" }
                                        button {
                                            class: "text-muted-foreground hover:text-destructive shrink-0",
                                            onclick: move |_| { iframe_cookies.write().remove(i); },
                                            Icon { name: "x", size: "14" }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "flex gap-2 items-end",
                            div { class: "flex flex-col gap-1 w-36",
                                Label { html_for: "ick", "Key" }
                                Input {
                                    id: "ick",
                                    placeholder: "session",
                                    value: iframe_new_cookie_key(),
                                    oninput: move |v| iframe_new_cookie_key.set(v),
                                }
                            }
                            div { class: "flex flex-col gap-1 flex-1",
                                Label { html_for: "icv", "Value" }
                                Input {
                                    id: "icv",
                                    placeholder: "abc123",
                                    value: iframe_new_cookie_val(),
                                    oninput: move |v| iframe_new_cookie_val.set(v),
                                }
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                onclick: move |_| {
                                    let k = iframe_new_cookie_key();
                                    let v = iframe_new_cookie_val();
                                    if !k.trim().is_empty() {
                                        iframe_cookies.write().push(KvEntry { key: k, value: v });
                                        iframe_new_cookie_key.set(String::new());
                                        iframe_new_cookie_val.set(String::new());
                                    }
                                },
                                "Add"
                            }
                        }
                    }

                    // Local Storage
                    div { class: "flex flex-col gap-2",
                        p { class: "text-sm font-medium", "Local Storage" }
                        if !iframe_local_storage.read().is_empty() {
                            div { class: "flex flex-col divide-y rounded-lg border",
                                for (i, entry) in iframe_local_storage.read().iter().enumerate() {
                                    div { class: "flex items-center gap-2 px-3 py-2 text-sm",
                                        span { class: "font-mono text-xs text-muted-foreground w-32 truncate", "{entry.key}" }
                                        span { class: "flex-1 text-xs truncate", "{entry.value}" }
                                        button {
                                            class: "text-muted-foreground hover:text-destructive shrink-0",
                                            onclick: move |_| { iframe_local_storage.write().remove(i); },
                                            Icon { name: "x", size: "14" }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "flex gap-2 items-end",
                            div { class: "flex flex-col gap-1 w-36",
                                Label { html_for: "ilsk", "Key" }
                                Input {
                                    id: "ilsk",
                                    placeholder: "theme",
                                    value: iframe_new_ls_key(),
                                    oninput: move |v| iframe_new_ls_key.set(v),
                                }
                            }
                            div { class: "flex flex-col gap-1 flex-1",
                                Label { html_for: "ilsv", "Value" }
                                Input {
                                    id: "ilsv",
                                    placeholder: "dark",
                                    value: iframe_new_ls_val(),
                                    oninput: move |v| iframe_new_ls_val.set(v),
                                }
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                onclick: move |_| {
                                    let k = iframe_new_ls_key();
                                    let v = iframe_new_ls_val();
                                    if !k.trim().is_empty() {
                                        iframe_local_storage.write().push(KvEntry { key: k, value: v });
                                        iframe_new_ls_key.set(String::new());
                                        iframe_new_ls_val.set(String::new());
                                    }
                                },
                                "Add"
                            }
                        }
                    }
                }
            }

            // Type-specific - Clock
            if type_key == "clock" {
                div { class: "flex flex-col gap-3",
                    if !clocks.read().is_empty() {
                        div { class: "flex flex-col divide-y rounded-lg border",
                            for (i, clock) in clocks.read().iter().enumerate() {
                                div { class: "flex items-center gap-3 px-3 py-2 text-sm",
                                    span { class: "flex-1 font-mono text-xs", "{clock.timezone}" }
                                    if let Some(lbl) = &clock.label {
                                        span { class: "text-muted-foreground", "{lbl}" }
                                    }
                                    span { class: "text-xs text-muted-foreground",
                                        if matches!(clock.style, ClockStyle::Digital) { "Digital" } else { "Analog" }
                                    }
                                    button {
                                        class: "text-muted-foreground hover:text-destructive",
                                        onclick: move |_| { clocks.write().remove(i); },
                                        Icon { name: "x", size: "14" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex gap-2 items-end flex-wrap",
                        div { class: "flex flex-col gap-1",
                            Label { html_for: "ctz", "Timezone" }
                            select {
                                id: "ctz",
                                class: "border-input flex h-9 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none min-w-56",
                                value: clock_new_tz(),
                                oninput: move |e| clock_new_tz.set(e.value()),
                                for (region, tzs) in tz_groups.read().iter() {
                                    optgroup { label: "{region}",
                                        for tz in tzs {
                                            option { value: "{tz}", "{tz}" }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "flex flex-col gap-1 w-28",
                            Label { html_for: "clabel", "Label (opt.)" }
                            Input {
                                id: "clabel",
                                placeholder: "Paris",
                                value: clock_new_label(),
                                oninput: move |v| clock_new_label.set(v),
                            }
                        }
                        div { class: "flex flex-col gap-1",
                            Label { html_for: "cstyle", "Style" }
                            select {
                                id: "cstyle",
                                class: "border-input flex h-9 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
                                oninput: move |e| clock_new_style.set(e.value()),
                                option { value: "digital", "Digital" }
                                option { value: "analog", "Analog" }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let tz = clock_new_tz();
                                if !tz.trim().is_empty() {
                                    let label = clock_new_label();
                                    let style = if clock_new_style() == "analog" { ClockStyle::Analog } else { ClockStyle::Digital };
                                    clocks.write().push(ClockConfig {
                                        timezone: tz,
                                        label: if label.is_empty() { None } else { Some(label) },
                                        style,
                                    });
                                    clock_new_tz.set("Europe/Paris".to_string());
                                    clock_new_label.set(String::new());
                                }
                            },
                            "Add clock"
                        }
                    }
                }
            }

            // Actions
            div { class: "flex gap-2 pt-2 border-t",
                Button {
                    onclick: move |_| do_save(),
                    "Save slide"
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
            }
        }
    }
}
