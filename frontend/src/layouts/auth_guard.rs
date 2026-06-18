use crate::auth;
use crate::routes::Route;
use dioxus::prelude::*;

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

    if has_token {
        rsx! { Outlet::<Route> {} }
    } else {
        rsx! {}
    }
}
