use crate::components::Icon;
use crate::ThemeMode;
use dioxus::prelude::*;

#[component]
pub fn ThemeToggle() -> Element {
    let mut theme = consume_context::<Signal<ThemeMode>>();

    let (icon, title) = match theme() {
        ThemeMode::System => ("monitor", "System"),
        ThemeMode::Light => ("sun", "Light"),
        ThemeMode::Dark => ("moon", "Dark"),
    };

    rsx! {
        button {
            title,
            class: "inline-flex h-9 w-9 items-center justify-center rounded-md transition-colors hover:bg-accent hover:text-accent-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
            onclick: move |_| {
                theme.set(match theme() {
                    ThemeMode::System => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                    ThemeMode::Dark => ThemeMode::System,
                });
            },
            Icon { name: icon, size: "18" }
        }
    }
}
