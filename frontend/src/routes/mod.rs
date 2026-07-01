use dioxus::prelude::*;

mod home;
mod login;
mod not_found;
mod screen;
mod screen_editor;
mod screens;
mod settings;
mod users;

pub use home::Home;
pub use login::Login;
pub use not_found::NotFound;
pub use screen::Screen;
pub use screen_editor::ScreenEditor;
pub use screens::Screens;
pub use settings::Settings;
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
    #[route("/screens")]
    Screens {},
    #[route("/screens/:id")]
    ScreenEditor { id: String },
    #[route("/users")]
    Users {},
    #[route("/settings")]
    Settings {},
    #[end_layout]
    #[end_layout]
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}
