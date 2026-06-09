use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct CardProps {
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn Card(props: CardProps) -> Element {
    rsx! {
        div {
            class: format!(
                "bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm {}",
                props.class
            ),
            {props.children}
        }
    }
}

#[component]
pub fn CardHeader(props: CardProps) -> Element {
    rsx! {
        div {
            class: format!("flex flex-col gap-1.5 px-6 {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn CardTitle(props: CardProps) -> Element {
    rsx! {
        h3 {
            class: format!("leading-none font-semibold {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn CardDescription(props: CardProps) -> Element {
    rsx! {
        p {
            class: format!("text-muted-foreground text-sm {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn CardContent(props: CardProps) -> Element {
    rsx! {
        div {
            class: format!("px-6 {}", props.class),
            {props.children}
        }
    }
}

#[component]
pub fn CardFooter(props: CardProps) -> Element {
    rsx! {
        div {
            class: format!("flex items-center px-6 {}", props.class),
            {props.children}
        }
    }
}
