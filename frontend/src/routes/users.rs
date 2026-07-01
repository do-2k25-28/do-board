use crate::auth;
use crate::components::{Button, ButtonVariant, Input, Label};
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_net::http::Request;
use shared::{CreateUserRequest, SetPasswordRequest, User};

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

#[component]
pub fn Users() -> Element {
    let mut users: Signal<Vec<User>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut fetch_error = use_signal(|| None::<String>);

    let mut show_create_form = use_signal(|| false);
    let mut new_email = use_signal(String::new);
    let mut new_password = use_signal(String::new);
    let mut create_error = use_signal(|| None::<String>);
    let mut creating = use_signal(|| false);
    let nav = use_navigator();

    use_effect(move || {
        spawn(async move {
            let token = match auth::get_token() {
                Some(t) => t,
                None => return,
            };

            match Request::get(&format!("{API_BASE}/api/users"))
                .header("Authorization", &format!("Bearer {token}"))
                .send()
                .await
            {
                Ok(resp) if resp.ok() => match resp.json::<Vec<User>>().await {
                    Ok(data) => {
                        users.set(data);
                        fetch_error.set(None);
                    }
                    Err(_) => fetch_error.set(Some("Failed to parse users.".to_string())),
                },
                Ok(resp) if resp.status() == 401 => {
                    auth::logout();
                    nav.replace(Route::Login {});
                    return;
                }
                Ok(_) => fetch_error.set(Some("Unauthorized.".to_string())),
                Err(_) => fetch_error.set(Some("Cannot reach server.".to_string())),
            }
            loading.set(false);
        });
    });

    let on_create_user = move |e: Event<FormData>| {
        e.prevent_default();
        let email_val = new_email();
        let pass_val = new_password();

        if email_val.is_empty() || pass_val.is_empty() {
            create_error.set(Some("Please fill in all fields.".to_string()));
            return;
        }

        spawn(async move {
            creating.set(true);
            create_error.set(None);

            let token = match auth::get_token() {
                Some(t) => t,
                None => {
                    create_error.set(Some("Not authenticated.".to_string()));
                    creating.set(false);
                    return;
                }
            };

            let body = match serde_json::to_string(&CreateUserRequest {
                email: email_val,
                password: pass_val,
            }) {
                Ok(b) => b,
                Err(_) => {
                    create_error.set(Some("Serialization error.".to_string()));
                    creating.set(false);
                    return;
                }
            };

            match Request::post(&format!("{API_BASE}/api/users"))
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {token}"))
                .body(body)
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => match resp.json::<User>().await {
                    Ok(new_user) => {
                        users.write().push(new_user);
                        new_email.set(String::new());
                        new_password.set(String::new());
                        show_create_form.set(false);
                    }
                    Err(_) => create_error.set(Some("Failed to parse response.".to_string())),
                },
                Ok(_) => create_error.set(Some("Email already exists or forbidden.".to_string())),
                Err(_) => create_error.set(Some("Cannot reach server.".to_string())),
            }

            creating.set(false);
        });
    };

    rsx! {
        div { class: "p-6",
            div { class: "flex items-center justify-between mb-6",
                div {
                    h1 { class: "text-2xl font-bold", "Users" }
                    p { class: "text-sm text-muted-foreground mt-0.5",
                        "All administrator accounts"
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| show_create_form.set(!show_create_form()),
                    if show_create_form() { "Cancel" } else { "Add user" }
                }
            }

            if show_create_form() {
                form {
                    onsubmit: on_create_user,
                    class: "mb-6 rounded-xl border bg-card p-4 flex flex-col gap-4 sm:flex-row sm:items-end",
                    div { class: "flex flex-col gap-2 flex-1",
                        Label { html_for: "new-email", "Email" }
                        Input {
                            id: "new-email",
                            input_type: "email",
                            placeholder: "user@example.com",
                            value: new_email(),
                            oninput: move |v| new_email.set(v),
                        }
                    }
                    div { class: "flex flex-col gap-2 flex-1",
                        Label { html_for: "new-password", "Password" }
                        Input {
                            id: "new-password",
                            input_type: "password",
                            placeholder: "••••••••",
                            value: new_password(),
                            oninput: move |v| new_password.set(v),
                        }
                    }
                    div { class: "flex flex-col justify-end gap-2",
                        if let Some(err) = create_error() {
                            p { class: "text-xs text-destructive", "{err}" }
                        }
                        Button {
                            disabled: creating(),
                            if creating() { "Creating…" } else { "Create account" }
                        }
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
            } else {
                div { class: "rounded-xl border overflow-hidden",
                    table { class: "w-full text-sm",
                        thead {
                            tr { class: "border-b bg-muted/40",
                                th { class: "text-left px-4 py-3 font-medium text-muted-foreground", "Email" }
                                th { class: "text-left px-4 py-3 font-medium text-muted-foreground hidden sm:table-cell", "Created" }
                                th { class: "text-right px-4 py-3 font-medium text-muted-foreground", "Actions" }
                            }
                        }
                        tbody {
                            for user in users.read().iter() {
                                UserRow { key: "{user.id}", user: user.clone() }
                            }
                            if users.read().is_empty() {
                                tr {
                                    td { class: "px-4 py-8 text-center text-muted-foreground", colspan: "3",
                                        "No users found."
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn UserRow(user: User) -> Element {
    let mut show_form = use_signal(|| false);
    let mut new_password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let user_id = use_signal(|| user.id.clone());

    let on_submit = move |e: Event<FormData>| {
        e.prevent_default();
        error.set(None);
        success.set(false);

        let new_pass = new_password();
        let confirm = confirm_password();

        if new_pass.is_empty() {
            error.set(Some("Please enter a new password.".to_string()));
            return;
        }
        if new_pass != confirm {
            error.set(Some("Passwords do not match.".to_string()));
            return;
        }

        spawn(async move {
            saving.set(true);

            let token = auth::get_token().unwrap_or_default();
            let id = user_id.read().clone();

            let body = match serde_json::to_string(&SetPasswordRequest {
                new_password: new_pass,
            }) {
                Ok(b) => b,
                Err(_) => {
                    error.set(Some("Serialization error.".to_string()));
                    saving.set(false);
                    return;
                }
            };

            match Request::put(&format!("{API_BASE}/api/users/{id}/password"))
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {token}"))
                .body(body)
                .unwrap()
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    new_password.set(String::new());
                    confirm_password.set(String::new());
                    success.set(true);
                }
                Ok(_) => error.set(Some("Failed to update password.".to_string())),
                Err(_) => error.set(Some("Cannot reach server.".to_string())),
            }

            saving.set(false);
        });
    };

    rsx! {
        tr { class: "border-b last:border-0 hover:bg-muted/20 transition-colors",
            td { class: "px-4 py-3 font-medium align-middle", "{user.email}" }
            td { class: "px-4 py-3 text-muted-foreground hidden sm:table-cell align-middle", "{user.created_at}" }
            td { class: "px-4 py-3 text-right align-top",
                div { class: "flex flex-col items-end gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            success.set(false);
                            error.set(None);
                            show_form.set(!show_form());
                        },
                        if show_form() { "Cancel" } else { "Change password" }
                    }

                    if show_form() {
                        form {
                            onsubmit: on_submit,
                            class: "flex flex-col gap-2 items-end w-64",
                            Input {
                                input_type: "password",
                                placeholder: "New password",
                                value: new_password(),
                                oninput: move |v| new_password.set(v),
                            }
                            Input {
                                input_type: "password",
                                placeholder: "Confirm new password",
                                value: confirm_password(),
                                oninput: move |v| confirm_password.set(v),
                            }
                            if let Some(err) = error() {
                                p { class: "text-xs text-destructive", "{err}" }
                            }
                            Button {
                                disabled: saving(),
                                if saving() { "Saving…" } else { "Save password" }
                            }
                        }
                    }

                    if success() {
                        p { class: "text-xs text-emerald-600", "Password updated." }
                    }
                }
            }
        }
    }
}
