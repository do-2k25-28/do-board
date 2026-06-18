use dioxus::prelude::*;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use shared::Device;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[component]
pub fn Home() -> Element {
    let mut devices: Signal<Vec<Device>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut fetch_error = use_signal(|| false);

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
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
            } else {
                div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4",
                    for device in devs.iter() {
                        DeviceCard { key: "{device.id}", device: device.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn DeviceCard(device: Device) -> Element {
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
            div { class: "text-xs text-muted-foreground space-y-0.5 pt-1 border-t",
                p { "Connected: {device.connected_at}" }
                p { "Last seen: {device.last_seen}" }
            }
        }
    }
}
