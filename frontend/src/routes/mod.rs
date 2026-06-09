use dioxus::prelude::*;

mod home;
mod login;
mod not_found;
mod screen;

pub use home::Home;
pub use login::Login;
pub use not_found::NotFound;
pub use screen::Screen;

use crate::layouts::main_layout::MainLayout;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    Screen {},
    #[route("/login")]
    Login {},
    #[layout(MainLayout)]
    #[route("/dashboard")]
    Home {},
    #[end_layout]
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}
