use crate::components::{
    Icon, Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarHeader, ThemeToggle,
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
                    }
                }
                SidebarFooter {
                    span { class: "text-xs text-muted-foreground", "{APP_NAME} v{APP_VERSION}" }
                }
            }
            div { class: "flex flex-1 flex-col overflow-hidden",
                header { class: "flex h-14 shrink-0 items-center justify-end gap-3 border-b bg-background text-foreground px-6",
                    ThemeToggle {}
                    img {
                        src: "https://ui-avatars.com/api/?name=Admin&background=1e293b&color=fff&rounded=true",
                        alt: "Profile",
                        class: "h-8 w-8 rounded-full",
                    }
                }
                main { class: "flex-1 overflow-auto",
                    Outlet::<Route> {}
                }
            }
        }
    }
}
