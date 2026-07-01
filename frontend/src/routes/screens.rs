use crate::auth;
use crate::components::{Button, ButtonVariant, Icon, Input, Label};
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_net::http::Request;
use shared::{CreateScreenRequest, Screen, SlideConfig};
use web_sys::RequestCredentials;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

fn slide_type_label(config: &SlideConfig) -> &'static str {
    match config {
        SlideConfig::Weather { .. } => "Weather",
        SlideConfig::Transport { .. } => "Transport",
        SlideConfig::Birthdays { .. } => "Birthdays",
        SlideConfig::Iframe { .. } => "iFrame",
        SlideConfig::Clock { .. } => "Clock",
        SlideConfig::Image { .. } => "Image",
        SlideConfig::Video { .. } => "Video",
    }
}

fn slide_type_icon(config: &SlideConfig) -> &'static str {
    match config {
        SlideConfig::Weather { .. } => "cloud-sun",
        SlideConfig::Transport { .. } => "bus",
        SlideConfig::Birthdays { .. } => "cake",
        SlideConfig::Iframe { .. } => "globe",
        SlideConfig::Clock { .. } => "clock",
        SlideConfig::Image { .. } => "image",
        SlideConfig::Video { .. } => "video",
    }
}

#[component]
pub fn Screens() -> Element {
    let screens: Signal<Vec<Screen>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let fetch_error = use_signal(|| None::<String>);
    let mut show_create = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut creating = use_signal(|| false);
    let nav = use_navigator();

    use_effect(move || {
        spawn(async move {
            fetch_screens(screens, fetch_error, loading, nav).await;
        });
    });

    let on_create = move |_| {
        let name = new_name();
        if name.trim().is_empty() {
            return;
        }
        spawn(async move {
            creating.set(true);
            let result = Request::post(&format!("{API_BASE}/api/screens"))
                .credentials(RequestCredentials::Include)
                .json(&CreateScreenRequest { name })
                .unwrap()
                .send()
                .await;
            creating.set(false);
            if let Ok(resp) = result {
                if let Ok(screen) = resp.json::<Screen>().await {
                    new_name.set(String::new());
                    show_create.set(false);
                    nav.push(Route::ScreenEditor { id: screen.id });
                }
            }
        });
    };

    let on_delete = move |id: String| {
        spawn(async move {
            let _ = Request::delete(&format!("{API_BASE}/api/screens/{id}"))
                .credentials(RequestCredentials::Include)
                .send()
                .await;
            fetch_screens(screens, fetch_error, loading, nav).await;
        });
    };

    let on_set_default = move |id: String| {
        spawn(async move {
            let _ = Request::put(&format!("{API_BASE}/api/screens/{id}/set-default"))
                .credentials(RequestCredentials::Include)
                .send()
                .await;
            fetch_screens(screens, fetch_error, loading, nav).await;
        });
    };

    rsx! {
        div { class: "p-6",
            div { class: "flex items-center justify-between mb-6",
                div {
                    h1 { class: "text-2xl font-bold", "Screens" }
                    p { class: "text-sm text-muted-foreground mt-0.5",
                        "Manage slideshow screens for your devices"
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| show_create.set(!show_create()),
                    if show_create() { "Cancel" } else { "New screen" }
                }
            }

            if show_create() {
                div { class: "mb-6 rounded-xl border bg-card p-4 flex gap-3 items-end",
                    div { class: "flex flex-col gap-2 flex-1",
                        Label { html_for: "screen-name", "Screen name" }
                        Input {
                            id: "screen-name",
                            placeholder: "Living room, Reception…",
                            value: new_name(),
                            oninput: move |v| new_name.set(v),
                        }
                    }
                    Button {
                        disabled: creating(),
                        onclick: on_create,
                        if creating() { "Creating…" } else { "Create & edit" }
                    }
                }
            }

            if let Some(err) = fetch_error() {
                div { class: "rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive mb-4",
                    "{err}"
                }
            }

            if loading() {
                div { class: "flex items-center justify-center p-12",
                    p { class: "text-muted-foreground text-sm", "Loading…" }
                }
            } else if screens.read().is_empty() {
                div { class: "flex flex-col items-center justify-center p-16 border rounded-lg border-dashed",
                    Icon { name: "presentation", size: "40" }
                    p { class: "font-medium mt-4", "No screens yet" }
                    p { class: "text-sm text-muted-foreground mt-1",
                        "Create a screen and configure its slides"
                    }
                }
            } else {
                div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
                    for screen in screens.read().iter() {
                        ScreenCard {
                            key: "{screen.id}",
                            screen: screen.clone(),
                            on_delete: {
                                let id = screen.id.clone();
                                move |_| on_delete(id.clone())
                            },
                            on_set_default: {
                                let id = screen.id.clone();
                                move |_| on_set_default(id.clone())
                            },
                        }
                    }
                }
            }
        }
    }
}

async fn fetch_screens(
    mut screens: Signal<Vec<Screen>>,
    mut fetch_error: Signal<Option<String>>,
    mut loading: Signal<bool>,
    nav: dioxus::router::Navigator,
) {
    match Request::get(&format!("{API_BASE}/api/screens"))
        .credentials(RequestCredentials::Include)
        .send()
        .await
    {
        Ok(resp) if resp.ok() => match resp.json::<Vec<Screen>>().await {
            Ok(data) => {
                screens.set(data);
                fetch_error.set(None);
            }
            Err(_) => fetch_error.set(Some("Failed to parse response.".into())),
        },
        Ok(resp) if resp.status() == 401 => {
            auth::logout().await;
            nav.replace(Route::Login {});
            return;
        }
        Ok(_) => fetch_error.set(Some("Unauthorized.".into())),
        Err(_) => fetch_error.set(Some("Cannot reach server.".into())),
    }
    loading.set(false);
}

#[component]
fn ScreenCard(
    screen: Screen,
    on_delete: EventHandler<()>,
    on_set_default: EventHandler<()>,
) -> Element {
    let nav = use_navigator();
    let screen_id = use_signal(|| screen.id.clone());
    let slide_count = screen.slides.len();
    let is_default = screen.is_default;

    rsx! {
        div { class: "border rounded-xl bg-card p-4 flex flex-col gap-3 hover:shadow-md transition-shadow",
            div { class: "flex items-start justify-between gap-2",
                div { class: "flex items-center gap-2 flex-wrap",
                    h3 { class: "font-semibold text-base", "{screen.name}" }
                    if is_default {
                        span { class: "inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary",
                            Icon { name: "star", size: "10" }
                            "Default"
                        }
                    }
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    size: crate::components::ButtonSize::IconXs,
                    onclick: move |_| on_delete.call(()),
                    Icon { name: "trash-2", size: "14" }
                }
            }

            p { class: "text-xs text-muted-foreground",
                if slide_count == 1 { "1 slide" } else { "{slide_count} slides" }
            }

            if !screen.slides.is_empty() {
                div { class: "flex flex-wrap gap-1.5",
                    for slide in screen.slides.iter() {
                        span { class: "inline-flex items-center gap-1 rounded-md border bg-muted/50 px-2 py-0.5 text-xs font-medium",
                            Icon { name: slide_type_icon(&slide.config), size: "11" }
                            "{slide_type_label(&slide.config)}"
                        }
                    }
                }
            }

            div { class: "flex gap-2 flex-wrap",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| { nav.push(Route::ScreenEditor { id: screen_id.read().clone() }); },
                    Icon { name: "pencil", size: "14" }
                    "Edit"
                }
                if !is_default {
                    Button {
                        variant: ButtonVariant::Ghost,
                        onclick: move |_| on_set_default.call(()),
                        Icon { name: "star", size: "14" }
                        "Set as default"
                    }
                }
            }
        }
    }
}
