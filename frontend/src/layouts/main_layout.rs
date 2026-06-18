use crate::auth;
use crate::components::{
    Button, ButtonVariant, Icon, Sidebar, SidebarContent, SidebarFooter, SidebarGroup,
    SidebarHeader, ThemeToggle,
};
use crate::routes::Route;
use crate::{APP_NAME, APP_VERSION};
use dioxus::prelude::*;

const NAV_ITEM: &str = "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground";
const NAV_ITEM_ACTIVE: &str = "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium bg-sidebar-accent text-sidebar-accent-foreground";

#[component]
pub fn MainLayout() -> Element {
    let route = use_route::<Route>();
    let is_devices = matches!(route, Route::Home {});
    let is_users = matches!(route, Route::Users {});
    let nav = use_navigator();

    let email = auth::get_email().unwrap_or_else(|| "Admin".to_string());
    let initials = email
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "A".to_string());

    let on_logout = move |_| {
        auth::logout();
        nav.replace(Route::Login {});
    };

    rsx! {
        div { class: "flex h-screen bg-background",
            Sidebar {
                SidebarHeader {
                    span { class: "font-semibold text-sm", "{APP_NAME}" }
                }
                SidebarContent {
                    SidebarGroup {
                        Link {
                            to: Route::Home {},
                            class: if is_devices { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            Icon { name: "monitor-dot", size: "18" }
                            "Devices"
                        }
                        Link {
                            to: Route::Users {},
                            class: if is_users { NAV_ITEM_ACTIVE } else { NAV_ITEM },
                            Icon { name: "users", size: "18" }
                            "Users"
                        }
                    }
                }
                SidebarFooter {
                    span { class: "text-xs text-muted-foreground", "{APP_NAME} v{APP_VERSION}" }
                }
            }
            div { class: "flex flex-1 flex-col overflow-hidden",
                header { class: "flex h-14 shrink-0 items-center justify-end gap-3 border-b bg-background text-foreground px-6",
                    ThemeToggle {}
                    Button {
                        variant: ButtonVariant::Ghost,
                        onclick: on_logout,
                        Icon { name: "log-out", size: "16" }
                        "Logout"
                    }
                    div { class: "flex items-center gap-2",
                        div { class: "flex h-8 w-8 items-center justify-center rounded-full bg-primary text-primary-foreground text-sm font-semibold",
                            "{initials}"
                        }
                        span { class: "text-sm font-medium hidden sm:block", "{email}" }
                    }
                }
                main { class: "flex-1 overflow-auto",
                    Outlet::<Route> {}
                }
            }
        }
    }
}
