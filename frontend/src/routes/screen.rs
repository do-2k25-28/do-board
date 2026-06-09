use dioxus::prelude::*;

#[component]
pub fn Screen() -> Element {
    rsx! {
        div { class: "p-6",
            h1 { class: "text-2xl font-bold", "Screen" }
            p { class: "text-muted-foreground mt-1", "Screen display." }
        }
    }
}
