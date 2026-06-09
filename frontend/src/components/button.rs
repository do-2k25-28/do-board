use super::icon::Icon;
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
}

#[derive(Clone, PartialEq, Default)]
pub enum ButtonSize {
    #[default]
    Default,
    Xs,
    Sm,
    Lg,
    Icon,
    IconXs,
    IconSm,
    IconLg,
}

#[derive(Props, Clone, PartialEq)]
pub struct ButtonProps {
    #[props(default)]
    pub variant: ButtonVariant,
    #[props(default)]
    pub size: ButtonSize,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub left_icon: Option<&'static str>,
    #[props(default)]
    pub right_icon: Option<&'static str>,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    pub children: Element,
}

#[component]
pub fn Button(props: ButtonProps) -> Element {
    let base = "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50";

    let variant_class = match props.variant {
        ButtonVariant::Default => {
            "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90"
        }
        ButtonVariant::Destructive => {
            "bg-destructive text-white shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/20"
        }
        ButtonVariant::Outline => {
            "border border-input bg-background shadow-xs hover:bg-accent hover:text-accent-foreground"
        }
        ButtonVariant::Secondary => {
            "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80"
        }
        ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
        ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
    };

    let size_class = match props.size {
        ButtonSize::Default => "h-9 px-4 py-2",
        ButtonSize::Xs => "h-7 rounded-md px-2",
        ButtonSize::Sm => "h-8 rounded-md px-3 text-xs",
        ButtonSize::Lg => "h-10 rounded-md px-6",
        ButtonSize::Icon => "size-9",
        ButtonSize::IconXs => "size-7",
        ButtonSize::IconSm => "size-8",
        ButtonSize::IconLg => "size-10",
    };

    let class = format!("{base} {variant_class} {size_class} {}", props.class);

    rsx! {
        button {
            class,
            disabled: props.disabled,
            onclick: move |e| {
                if let Some(handler) = &props.onclick {
                    handler.call(e);
                }
            },
            if let Some(name) = props.left_icon {
                Icon { name, size: "16" }
            }
            {props.children}
            if let Some(name) = props.right_icon {
                Icon { name, size: "16" }
            }
        }
    }
}
