use crate::components::{
    Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, Label,
};
use dioxus::prelude::*;

#[component]
pub fn Login() -> Element {
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);

    let onsubmit = move |e: Event<FormData>| {
        e.prevent_default();
        // TODO: login
    };

    rsx! {
        div { class: "flex h-screen items-center justify-center bg-background",
            Card { class: "w-full max-w-sm",
                CardHeader {
                    CardTitle { "Login" }
                    CardDescription { "Access administration dashboard." }
                }
                CardContent {
                    form { onsubmit,
                        div { class: "flex flex-col gap-4",
                            div { class: "flex flex-col gap-2",
                                Label { html_for: "email", "Email" }
                                Input {
                                    id: "email",
                                    input_type: "email",
                                    placeholder: "admin@example.com",
                                    value: email(),
                                    oninput: move |v| email.set(v),
                                }
                            }
                            div { class: "flex flex-col gap-2",
                                Label { html_for: "password", "Password" }
                                Input {
                                    id: "password",
                                    input_type: "password",
                                    placeholder: "••••••••",
                                    value: password(),
                                    oninput: move |v| password.set(v),
                                }
                            }
                            Button { class: "w-full", "Login" }
                        }
                    }
                }
            }
        }
    }
}
