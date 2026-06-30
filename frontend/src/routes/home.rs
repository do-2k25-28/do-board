use dioxus::prelude::*;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use shared::{Device, Screen};

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[component]
pub fn Home() -> Element {
    let mut devices: Signal<Vec<Device>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut fetch_error = use_signal(|| false);
    let mut refresh = use_signal(|| 0u32);
    let mut search = use_signal(String::new);
    let filter_status = use_signal(|| "all");
    let filter_saved = use_signal(|| "all");

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            let _ = refresh();
            match Request::get(&format!("{API_BASE}/api/devices"))
                .send()
                .await
            {
                Ok(resp) => match resp.json::<Vec<Device>>().await {
                    Ok(data) => {
                        devices.set(data);
                        fetch_error.set(false);
                    }
                    Err(_) => fetch_error.set(true),
                },
                Err(_) => fetch_error.set(true),
            }
            loading.set(false);
            TimeoutFuture::new(5_000).await;
        }
    });

    let devs = devices.read();
    let online_count = devs.iter().filter(|d| d.online).count();
    let total_count = devs.len();

    let search_val = search().to_lowercase();
    let status_val = filter_status();
    let saved_val = filter_saved();

    let filtered: Vec<&Device> = devs
        .iter()
        .filter(|d| {
            let search_ok = search_val.is_empty()
                || d.ip.to_lowercase().contains(&search_val)
                || d.browser.to_lowercase().contains(&search_val)
                || d.os.to_lowercase().contains(&search_val)
                || d.name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_val);
            let status_ok = match status_val {
                "online" => d.online,
                "offline" => !d.online,
                _ => true,
            };
            let saved_ok = match saved_val {
                "saved" => d.saved,
                "unsaved" => !d.saved,
                _ => true,
            };
            search_ok && status_ok && saved_ok
        })
        .collect();

    rsx! {
        div { class: "p-6",
            div { class: "flex items-center justify-between mb-6",
                div {
                    h1 { class: "text-2xl font-bold", "Devices" }
                    p { class: "text-sm text-muted-foreground mt-0.5",
                        "Updated every 5 seconds"
                    }
                }
                div { class: "flex items-center gap-2 text-sm",
                    span { class: "inline-flex items-center gap-1.5 rounded-full px-3 py-1 bg-green-500/10 text-green-600 dark:text-green-400 font-medium",
                        span { class: "w-1.5 h-1.5 rounded-full bg-green-500" }
                        "{online_count} online"
                    }
                    span { class: "text-muted-foreground", "/ {total_count} total" }
                }
            }

            if fetch_error() {
                div { class: "rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive mb-4",
                    "Cannot reach backend"
                }
            }

            div { class: "flex flex-col sm:flex-row gap-3", style: "margin-bottom: 2rem",
                input {
                    class: "flex-1 text-sm border rounded-lg px-3 py-2 bg-background focus:outline-none focus:ring-1 focus:ring-ring",
                    r#type: "text",
                    placeholder: "Search by name, IP, browser, OS…",
                    value: "{search}",
                    oninput: move |e| search.set(e.value()),
                }
                div { class: "flex gap-2 flex-wrap",
                    FilterGroup {
                        options: vec![("all", "All"), ("online", "Online"), ("offline", "Offline")],
                        value: filter_status,
                    }
                    FilterGroup {
                        options: vec![("all", "All"), ("saved", "Saved"), ("unsaved", "Unsaved")],
                        value: filter_saved,
                    }
                }
            }

            if loading() {
                div { class: "flex items-center justify-center p-12",
                    p { class: "text-muted-foreground text-sm", "Loading…" }
                }
            } else if devs.is_empty() {
                div { class: "flex flex-col items-center justify-center p-16 border rounded-lg border-dashed",
                    p { class: "font-medium", "No devices" }
                    p { class: "text-sm text-muted-foreground mt-1",
                        "Clients connecting to \"/\" will appear here"
                    }
                }
            } else if filtered.is_empty() {
                div { class: "flex flex-col items-center justify-center p-16 border rounded-lg border-dashed",
                    p { class: "font-medium", "No results" }
                    p { class: "text-sm text-muted-foreground mt-1", "Try adjusting your search or filters" }
                }
            } else {
                div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4",
                    for device in filtered.iter() {
                        DeviceCard {
                            key: "{device.id}",
                            device: (*device).clone(),
                            on_saved: move |_| refresh.set(refresh() + 1),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FilterGroup(options: Vec<(&'static str, &'static str)>, value: Signal<&'static str>) -> Element {
    rsx! {
        div { class: "inline-flex rounded-lg border bg-muted/40 p-0.5 gap-0.5",
            for (key, label) in options {
                button {
                    class: if value() == key {
                        "text-xs px-3 py-1 rounded-md bg-background shadow-sm font-medium transition-colors"
                    } else {
                        "text-xs px-3 py-1 rounded-md text-muted-foreground hover:text-foreground transition-colors"
                    },
                    onclick: move |_| value.set(key),
                    "{label}"
                }
            }
        }
    }
}

#[component]
fn DeviceCard(device: Device, on_saved: EventHandler<()>) -> Element {
    let mut show_save_form = use_signal(|| false);
    let is_saved = device.saved;
    let is_online = device.online;
    let name_init = device.name.clone();
    let mut save_name = use_signal(|| device.name.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut saved = use_signal(move || is_saved);
    let mut display_name: Signal<Option<String>> = use_signal(move || name_init);
    let device_id = use_signal(|| device.id.clone());

    // Push screen state
    let mut push_open = use_signal(|| false);
    let mut push_screens: Signal<Vec<Screen>> = use_signal(Vec::new);
    let mut push_screen_id: Signal<Option<String>> = use_signal(|| None);
    let mut pushing = use_signal(|| false);

    let do_save = move || {
        let id = device_id.read().clone();
        let name_val = save_name();
        if name_val.trim().is_empty() {
            return;
        }
        spawn(async move {
            saving.set(true);
            let token = crate::auth::get_token().unwrap_or_default();
            let result = Request::post(&format!("{API_BASE}/api/devices/{id}/save"))
                .header("Authorization", &format!("Bearer {token}"))
                .json(&shared::SaveDeviceRequest {
                    name: name_val.clone(),
                })
                .unwrap()
                .send()
                .await;
            saving.set(false);
            if result.is_ok() {
                saved.set(true);
                display_name.set(Some(name_val));
                show_save_form.set(false);
                on_saved.call(());
            }
        });
    };

    let mut on_push_toggle = move || {
        let was_open = push_open();
        push_open.set(!was_open);
        if !was_open {
            // Opening - fetch available screens
            spawn(async move {
                let token = crate::auth::get_token().unwrap_or_default();
                if let Ok(resp) = Request::get(&format!("{API_BASE}/api/screens"))
                    .header("Authorization", &format!("Bearer {token}"))
                    .send()
                    .await
                {
                    if let Ok(screens) = resp.json::<Vec<Screen>>().await {
                        push_screens.set(screens);
                        push_screen_id.set(None);
                    }
                }
            });
        }
    };

    let do_push = move || {
        let Some(screen_id) = push_screen_id() else {
            return;
        };
        let id = device_id.read().clone();
        spawn(async move {
            pushing.set(true);
            let token = crate::auth::get_token().unwrap_or_default();
            let _ = Request::post(&format!("{API_BASE}/api/devices/{id}/push-screen"))
                .header("Authorization", &format!("Bearer {token}"))
                .json(&shared::PushScreenRequest {
                    screen_id: screen_id.clone(),
                })
                .unwrap()
                .send()
                .await;
            pushing.set(false);
            push_open.set(false);
        });
    };

    rsx! {
        div {
            class: "border rounded-xl p-4 bg-card flex flex-col gap-3 transition-shadow hover:shadow-md",
            div { class: "flex items-center justify-between",
                div { class: "flex items-center gap-2",
                    span {
                        class: if device.online {
                            "w-2.5 h-2.5 rounded-full bg-green-500 shrink-0"
                        } else {
                            "w-2.5 h-2.5 rounded-full bg-gray-400 shrink-0"
                        }
                    }
                    span {
                        class: if device.online {
                            "text-sm font-semibold text-green-600 dark:text-green-400"
                        } else {
                            "text-sm font-semibold text-muted-foreground"
                        },
                        if device.online { "Online" } else { "Offline" }
                    }
                }
                if !saved() {
                    button {
                        class: "text-xs px-2 py-1 rounded border border-border hover:bg-accent text-muted-foreground transition-colors",
                        onclick: move |_| show_save_form.set(!show_save_form()),
                        if show_save_form() { "Cancel" } else { "Save" }
                    }
                } else {
                    span { class: "text-xs text-muted-foreground italic", "Saved" }
                }
            }

            if let Some(name) = display_name() {
                p { class: "font-semibold text-base truncate", "{name}" }
            }

            p { class: "font-mono text-lg font-bold tracking-tight", "{device.ip}" }

            div { class: "flex flex-wrap gap-1.5",
                span { class: "inline-flex items-center rounded-md border bg-muted/50 px-2 py-0.5 text-xs font-medium",
                    "{device.browser}"
                }
                span { class: "inline-flex items-center rounded-md border bg-muted/50 px-2 py-0.5 text-xs font-medium",
                    "{device.os}"
                }
            }

            if show_save_form() {
                div { class: "flex gap-2 items-center",
                    input {
                        class: "flex-1 text-sm border rounded-md px-2 py-1 bg-background focus:outline-none focus:ring-1 focus:ring-ring",
                        r#type: "text",
                        placeholder: "Device name…",
                        value: "{save_name}",
                        oninput: move |e| save_name.set(e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                do_save();
                            }
                        },
                    }
                    button {
                        class: "text-xs px-3 py-1 rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 shrink-0",
                        disabled: saving(),
                        onclick: move |_| do_save(),
                        if saving() { "…" } else { "Confirm" }
                    }
                }
            }

            // Push screen panel (online devices only)
            if is_online {
                button {
                    class: "text-xs px-3 py-1.5 rounded-md border border-dashed border-border text-muted-foreground hover:bg-accent hover:text-foreground transition-colors w-full",
                    onclick: move |_| on_push_toggle(),
                    if push_open() { "Cancel" } else { "Push a screen →" }
                }

                if push_open() {
                    div { class: "flex flex-col gap-2",
                        if push_screens.read().is_empty() {
                            p { class: "text-xs text-muted-foreground text-center py-2",
                                "No screens available - create one first"
                            }
                        } else {
                            select {
                                class: "text-sm border rounded-md px-2 py-1.5 bg-background w-full focus:outline-none focus:ring-1 focus:ring-ring",
                                onchange: move |e| {
                                    let val = e.value();
                                    push_screen_id.set(if val.is_empty() { None } else { Some(val) });
                                },
                                option { value: "", "Select a screen…" }
                                for screen in push_screens.read().iter() {
                                    option { value: "{screen.id}", "{screen.name}" }
                                }
                            }
                            button {
                                class: "text-xs px-3 py-1.5 rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 w-full",
                                disabled: push_screen_id().is_none() || pushing(),
                                onclick: move |_| do_push(),
                                if pushing() { "Pushing…" } else { "Push to device" }
                            }
                        }
                    }
                }
            }

            div { class: "text-xs text-muted-foreground space-y-0.5 pt-1 border-t",
                p { "Connected: {device.connected_at}" }
                p { "Last seen: {device.last_seen}" }
            }
        }
    }
}
