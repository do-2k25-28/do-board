use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct LabelProps {
    #[props(default)]
    pub html_for: String,
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn Label(props: LabelProps) -> Element {
    rsx! {
        label {
            r#for: props.html_for,
            class: format!(
                "text-sm leading-none font-medium select-none peer-disabled:cursor-not-allowed peer-disabled:opacity-50 {}",
                props.class
            ),
            {props.children}
        }
    }
}
