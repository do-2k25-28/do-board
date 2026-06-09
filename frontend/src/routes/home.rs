use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "p-6",
            h1 { class: "text-2xl font-bold", "Dashboard" }
        }
    }
}
