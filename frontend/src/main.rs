use dioxus::prelude::*;

const TAILWIND: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: TAILWIND }
        main {
            h1 { class: "text-2xl font-bold", "Do-Board" }
            p { class: "text-gray-600", "Tableau de bord personnalisable" }
        }
    }
}
