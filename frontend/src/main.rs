use dioxus::prelude::*;

pub mod components;
mod layouts;
mod routes;

use routes::Route;

pub const APP_NAME: &str = env!("APP_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const TAILWIND: Asset = asset!("/assets/tailwind.css");

#[derive(Clone, PartialEq, Copy, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[cfg(target_arch = "wasm32")]
fn apply_theme(mode: ThemeMode) {
    use web_sys::window;
    let Some(root) = window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    else {
        return;
    };
    let cls = root.class_list();
    let _ = cls.remove_1("dark");
    let _ = cls.remove_1("light");
    match mode {
        ThemeMode::Dark => {
            let _ = cls.add_1("dark");
        }
        ThemeMode::Light => {
            let _ = cls.add_1("light");
        }
        ThemeMode::System => {}
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let theme = use_context_provider(|| Signal::new(ThemeMode::default()));

    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        apply_theme(theme());
        #[cfg(not(target_arch = "wasm32"))]
        let _ = theme();
    });

    rsx! {
        document::Link { rel: "stylesheet", href: TAILWIND }
        Router::<Route> {}
    }
}
