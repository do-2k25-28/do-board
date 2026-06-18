use dioxus::prelude::*;
use web_sys::WebSocket;

// TODO: configurable env var
const WS_URL: &str = "ws://localhost:3000/ws";

#[component]
pub fn Screen() -> Element {
    let _ws: Signal<Option<WebSocket>> = use_signal(|| WebSocket::new(WS_URL).ok());

    rsx! {
        div { class: "flex items-center justify-center h-screen bg-background",
            div { class: "text-center",
                h1 { class: "text-4xl font-bold text-foreground", "DO Board" }
                p { class: "text-muted-foreground mt-2", "Screen display connected" }
            }
        }
    }
}
