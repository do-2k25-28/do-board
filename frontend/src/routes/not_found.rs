use dioxus::prelude::*;

#[component]
pub fn NotFound(route: Vec<String>) -> Element {
    rsx! {
        div { class: "flex h-screen flex-col items-center justify-center gap-4",
            h1 { class: "text-6xl font-bold text-muted-foreground", "404" }
            p { class: "text-muted-foreground", "Page not found : /{route.join(\"/\")}" }
        }
    }
}
