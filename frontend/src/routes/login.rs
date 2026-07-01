use crate::auth;
use crate::components::{
    Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, Label,
};
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_net::http::Request;
use shared::{LoginRequest, LoginResponse};
use web_sys::RequestCredentials;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[component]
pub fn Login() -> Element {
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let nav = use_navigator();

    let onsubmit = move |e: Event<FormData>| {
        e.prevent_default();
        let email_val = email();
        let password_val = password();

        if email_val.is_empty() || password_val.is_empty() {
            error.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        spawn(async move {
            loading.set(true);
            error.set(None);

            let body = match serde_json::to_string(&LoginRequest {
                email: email_val,
                password: password_val,
            }) {
                Ok(b) => b,
                Err(_) => {
                    error.set(Some("Serialization error.".to_string()));
                    loading.set(false);
                    return;
                }
            };

            let result = Request::post(&format!("{API_BASE}/api/auth/login"))
                .header("Content-Type", "application/json")
                .credentials(RequestCredentials::Include)
                .body(body)
                .unwrap()
                .send()
                .await;

            match result {
                Ok(resp) if resp.ok() => match resp.json::<LoginResponse>().await {
                    Ok(data) => {
                        auth::set_email(&data.user.email);
                        auth::set_session(data.expires_at);
                        nav.replace(Route::Home {});
                    }
                    Err(_) => {
                        error.set(Some("Unexpected server response.".to_string()));
                    }
                },
                Ok(_) => {
                    error.set(Some("Invalid email or password.".to_string()));
                }
                Err(_) => {
                    error.set(Some("Cannot reach server. Please try again.".to_string()));
                }
            }

            loading.set(false);
        });
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
                            if let Some(err) = error() {
                                div { class: "rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive",
                                    "{err}"
                                }
                            }
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
                            Button {
                                class: "w-full",
                                disabled: loading(),
                                if loading() { "Signing in…" } else { "Login" }
                            }
                        }
                    }
                }
            }
        }
    }
}
