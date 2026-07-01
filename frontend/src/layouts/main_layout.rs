use crate::auth;
use crate::components::{
    Button, ButtonSize, ButtonVariant, Icon, Sidebar, SidebarContent, SidebarFooter, SidebarGroup,
    SidebarHeader, ThemeToggle,
};
use crate::routes::Route;
use crate::{APP_NAME, APP_VERSION};
use dioxus::prelude::*;

const NAV_ITEM: &str = "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground";
const NAV_ITEM_ACTIVE: &str = "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium bg-sidebar-accent text-sidebar-accent-foreground";

// Below `lg`, the sidebar is an off-canvas drawer toggled by the header
// button; at `lg` and above it's always shown as part of the static layout.
const SIDEBAR_CLOSED: &str = "fixed inset-y-0 left-0 z-40 -translate-x-full transition-transform duration-200 ease-in-out lg:static lg:translate-x-0";
const SIDEBAR_OPEN: &str = "fixed inset-y-0 left-0 z-40 translate-x-0 transition-transform duration-200 ease-in-out lg:static";

#[component]
pub fn MainLayout() -> Element {
    let route = use_route::<Route>();
    let is_devices = matches!(route, Route::Home {});
    let is_screens = matches!(route, Route::Screens {} | Route::ScreenEditor { .. });
    let is_users = matches!(route, Route::Users {});
    let is_settings = matches!(route, Route::Settings {});
    let nav = use_navigator();
    let mut sidebar_open = use_signal(|| false);

    let email = auth::get_email().unwrap_or_else(|| "Admin".to_string());
    let initials = email
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "A".to_string());

    let on_logout = move |_| {
        spawn(async move {
            auth::logout().await;
            nav.replace(Route::Login {});
        });
    };

    rsx! {
        div { class: "flex h-screen bg-background",
            if sidebar_open() {
                div {
                    class: "fixed inset-0 z-30 bg-black/50 lg:hidden",
                    onclick: move |_| sidebar_open.set(false),
                }
            }
            Sidebar {
                class: if sidebar_open() { SIDEBAR_OPEN } else { SIDEBAR_CLOSED },
                SidebarHeader {
                    span { class: "font-semibold text-sm", "{APP_NAME}" }
                }
                SidebarContent {
                    SidebarGroup {
                        Link {
                            to: Route::Home {},
                            class: if is_devices { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            onclick: move |_| sidebar_open.set(false),
                            Icon { name: "monitor-dot", size: "18" }
                            "Devices"
                        }
                        Link {
                            to: Route::Screens {},
                            class: if is_screens { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            onclick: move |_| sidebar_open.set(false),
                            Icon { name: "presentation", size: "18" }
                            "Screens"
                        }
                        Link {
                            to: Route::Users {},
                            class: if is_users { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            onclick: move |_| sidebar_open.set(false),
                            Icon { name: "users", size: "18" }
                            "Users"
                        }
                        Link {
                            to: Route::Settings {},
                            class: if is_settings { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            onclick: move |_| sidebar_open.set(false),
                            Icon { name: "settings", size: "18" }
                            "Settings"
                        }
                    }
                }
                SidebarFooter {
                    span { class: "text-xs text-muted-foreground", "{APP_NAME} v{APP_VERSION}" }
                }
            }
            div { class: "flex flex-1 flex-col overflow-hidden",
                header { class: "flex h-14 shrink-0 items-center gap-3 border-b bg-background text-foreground px-6",
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Icon,
                        class: "lg:hidden",
                        onclick: move |_| sidebar_open.set(!sidebar_open()),
                        Icon { name: "menu", size: "20" }
                    }
                    div { class: "flex items-center gap-3 ml-auto",
                        ThemeToggle {}
                        Button {
                            variant: ButtonVariant::Ghost,
                            onclick: on_logout,
                            Icon { name: "log-out", size: "16" }
                            "Logout"
                        }
                        Link {
                            to: Route::Settings {},
                            class: "flex items-center gap-2",
                            div { class: "flex h-8 w-8 items-center justify-center rounded-full bg-primary text-primary-foreground text-sm font-semibold",
                                "{initials}"
                            }
                            span { class: "text-sm font-medium hidden sm:block", "{email}" }
                        }
                    }
                }
                main { class: "flex-1 overflow-auto",
                    Outlet::<Route> {}
                }
            }
        }
    }
}
