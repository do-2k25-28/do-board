use crate::auth;
use crate::components::{
    Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, Label,
};
use dioxus::prelude::*;
use gloo_net::http::Request;
use shared::ChangePasswordRequest;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[component]
pub fn Settings() -> Element {
    let email = auth::get_email().unwrap_or_else(|| "Admin".to_string());

    let mut current_password = use_signal(String::new);
    let mut new_password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);
    let mut saving = use_signal(|| false);

    let on_submit = move |e: Event<FormData>| {
        e.prevent_default();
        error.set(None);
        success.set(false);

        let current = current_password();
        let new_pass = new_password();
        let confirm = confirm_password();

        if current.is_empty() || new_pass.is_empty() {
            error.set(Some("Please fill in all fields.".to_string()));
            return;
        }
        if new_pass != confirm {
            error.set(Some("New passwords do not match.".to_string()));
            return;
        }

        spawn(async move {
            saving.set(true);

            let token = match auth::get_token() {
                Some(t) => t,
                None => {
                    error.set(Some("Not authenticated.".to_string()));
                    saving.set(false);
                    return;
                }
            };

            let body = match serde_json::to_string(&ChangePasswordRequest {
                current_password: current,
                new_password: new_pass,
            }) {
                Ok(b) => b,
                Err(_) => {
                    error.set(Some("Serialization error.".to_string()));
                    saving.set(false);
                    return;
                }
            };

            match Request::put(&format!("{API_BASE}/api/users/me/password"))
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {token}"))
                .body(body)
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    current_password.set(String::new());
                    new_password.set(String::new());
                    confirm_password.set(String::new());
                    success.set(true);
                }
                Ok(resp) if resp.status() == 401 => {
                    error.set(Some("Current password is incorrect.".to_string()));
                }
                Ok(_) => error.set(Some("Failed to update password.".to_string())),
                Err(_) => error.set(Some("Cannot reach server.".to_string())),
            }

            saving.set(false);
        });
    };

    rsx! {
        div { class: "p-6 max-w-xl",
            div { class: "mb-6",
                h1 { class: "text-2xl font-bold", "Settings" }
                p { class: "text-sm text-muted-foreground mt-0.5", "{email}" }
            }

            Card {
                CardHeader {
                    CardTitle { "Change password" }
                    CardDescription { "Update the password used to sign in to this account." }
                }
                CardContent {
                    form {
                        onsubmit: on_submit,
                        class: "flex flex-col gap-4",
                        div { class: "flex flex-col gap-2",
                            Label { html_for: "current-password", "Current password" }
                            Input {
                                id: "current-password",
                                input_type: "password",
                                placeholder: "••••••••",
                                value: current_password(),
                                oninput: move |v| current_password.set(v),
                            }
                        }
                        div { class: "flex flex-col gap-2",
                            Label { html_for: "new-password", "New password" }
                            Input {
                                id: "new-password",
                                input_type: "password",
                                placeholder: "••••••••",
                                value: new_password(),
                                oninput: move |v| new_password.set(v),
                            }
                        }
                        div { class: "flex flex-col gap-2",
                            Label { html_for: "confirm-password", "Confirm new password" }
                            Input {
                                id: "confirm-password",
                                input_type: "password",
                                placeholder: "••••••••",
                                value: confirm_password(),
                                oninput: move |v| confirm_password.set(v),
                            }
                        }

                        if let Some(err) = error() {
                            p { class: "text-sm text-destructive", "{err}" }
                        }
                        if success() {
                            p { class: "text-sm text-emerald-600", "Password updated successfully." }
                        }

                        div { class: "flex justify-end",
                            Button {
                                disabled: saving(),
                                if saving() { "Saving…" } else { "Save password" }
                            }
                        }
                    }
                }
            }
        }
    }
}
