use crate::auth;
use crate::routes::Route;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[component]
pub fn AuthGuard() -> Element {
    let nav = use_navigator();
    let has_session = auth::has_session_hint();

    use_effect(move || {
        if !has_session {
            nav.replace(Route::Login {});
        }
    });

    // Proactively sign the user out once their session expires, even if
    // they stay idle on a dashboard page with no in-flight requests to fail.
    // Any API call hitting a real 401 (e.g. the cookie was rejected for a
    // reason this client-side hint can't see) also redirects on its own.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        if !has_session {
            return;
        }
        loop {
            if auth::is_session_expired() {
                auth::logout().await;
                nav.replace(Route::Login {});
                return;
            }
            TimeoutFuture::new(10_000).await;
        }
    });

    if has_session {
        rsx! { Outlet::<Route> {} }
    } else {
        rsx! {}
    }
}
