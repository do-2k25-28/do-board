use crate::auth;
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[component]
pub fn AuthGuard() -> Element {
    let nav = use_navigator();
    let token = auth::get_token();
    let has_token = token.is_some();

    use_effect(move || {
        if !has_token {
            nav.replace(Route::Login {});
        }
    });

    // Proactively sign the user out once their JWT expires, even if they
    // stay idle on a dashboard page with no in-flight requests to fail.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        if !has_token {
            return;
        }
        loop {
            if auth::is_token_expired() {
                auth::logout();
                nav.replace(Route::Login {});
                return;
            }
            TimeoutFuture::new(10_000).await;
        }
    });

    if has_token {
        rsx! { Outlet::<Route> {} }
    } else {
        rsx! {}
    }
}
