use super::icon::Icon;
use dioxus::prelude::*;

type NavItemOpen = Signal<bool>;

#[derive(Props, Clone, PartialEq)]
pub struct NavigationMenuProps {
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn NavigationMenu(props: NavigationMenuProps) -> Element {
    rsx! {
        nav {
            class: format!(
                "relative z-10 flex max-w-max flex-1 items-center justify-center {}",
                props.class
            ),
            {props.children}
        }
    }
}

#[component]
pub fn NavigationMenuList(props: NavigationMenuProps) -> Element {
    rsx! {
        ul {
            class: format!("flex flex-1 list-none items-center justify-center gap-1 {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn NavigationMenuItem(props: NavigationMenuProps) -> Element {
    let open: NavItemOpen = use_signal(|| false);
    provide_context(open);

    rsx! {
        li {
            class: format!("relative {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn NavigationMenuTrigger(props: NavigationMenuProps) -> Element {
    let mut open = consume_context::<NavItemOpen>();

    rsx! {
        button {
            class: format!(
                "inline-flex h-9 w-max items-center justify-center gap-1.5 rounded-md bg-background px-4 py-2 text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring {}",
                props.class
            ),
            onclick: move |_| open.toggle(),
            {props.children}
            Icon {
                name: "chevron-down",
                size: "14",
                class: if open() { "rotate-180 transition-transform duration-200" } else { "transition-transform duration-200" },
            }
        }
    }
}

#[component]
pub fn NavigationMenuContent(props: NavigationMenuProps) -> Element {
    let open = consume_context::<NavItemOpen>();

    rsx! {
        if open() {
            div {
                class: format!(
                    "absolute top-full left-0 mt-1.5 min-w-48 rounded-md border bg-popover text-popover-foreground p-1 shadow-md {}",
                    props.class
                ),
                {props.children}
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct NavigationMenuLinkProps {
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    pub children: Element,
}

#[component]
pub fn NavigationMenuLink(props: NavigationMenuLinkProps) -> Element {
    rsx! {
        button {
            class: format!(
                "block w-full select-none rounded-md p-3 text-sm leading-none text-left outline-none transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring {}",
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
