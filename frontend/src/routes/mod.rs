use dioxus::prelude::*;

mod home;
mod login;
mod not_found;
mod screen;
mod users;

pub use home::Home;
pub use login::Login;
pub use not_found::NotFound;
pub use screen::Screen;
pub use users::Users;

use crate::layouts::auth_guard::AuthGuard;
use crate::layouts::main_layout::MainLayout;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    Screen {},
    #[route("/login")]
    Login {},
    #[layout(AuthGuard)]
    #[layout(MainLayout)]
    #[route("/dashboard")]
    Home {},
    #[route("/users")]
    Users {},
    #[end_layout]
    #[end_layout]
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}
