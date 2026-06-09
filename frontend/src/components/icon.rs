use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IconProps {
    pub name: &'static str,
    #[props(default = "24")]
    pub size: &'static str,
    #[props(default)]
    pub class: String,
}

#[component]
pub fn Icon(props: IconProps) -> Element {
    rsx! {
        i {
            "data-lucide": props.name,
            width: props.size,
            height: props.size,
            class: props.class,
        }
    }
}
