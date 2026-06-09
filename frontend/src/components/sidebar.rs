use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct SidebarProps {
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn Sidebar(props: SidebarProps) -> Element {
    rsx! {
        aside {
            class: format!(
                "flex h-full w-64 flex-col bg-sidebar text-sidebar-foreground border-r border-sidebar-border {}",
                props.class
            ),
            {props.children}
        }
    }
}

#[component]
pub fn SidebarHeader(props: SidebarProps) -> Element {
    rsx! {
        div {
            class: format!("flex items-center px-4 py-3 border-b border-sidebar-border {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn SidebarContent(props: SidebarProps) -> Element {
    rsx! {
        div {
            class: format!("flex-1 overflow-auto py-2 {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn SidebarFooter(props: SidebarProps) -> Element {
    rsx! {
        div {
            class: format!("border-t border-sidebar-border px-4 py-3 {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn SidebarGroup(props: SidebarProps) -> Element {
    rsx! {
        div {
            class: format!("px-3 py-2 {}", props.class),
            {props.children}
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SidebarGroupLabelProps {
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn SidebarGroupLabel(props: SidebarGroupLabelProps) -> Element {
    rsx! {
        p {
            class: format!(
                "mb-1 px-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase {}",
                props.class
            ),
            {props.children}
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SidebarMenuItemProps {
    #[props(default)]
    pub active: bool,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    pub children: Element,
}

#[component]
pub fn SidebarMenuItem(props: SidebarMenuItemProps) -> Element {
    let active_class = if props.active {
        "bg-sidebar-accent text-sidebar-accent-foreground"
    } else {
        "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
    };

    rsx! {
        button {
            disabled: props.disabled,
            class: format!(
                "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 {} {}",
                active_class,
                props.class
            ),
            onclick: move |e| {
                if let Some(handler) = &props.onclick {
                    handler.call(e);
                }
            },
            {props.children}
        }
    }
}
