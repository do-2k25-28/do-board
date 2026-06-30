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
            // Omit the `for` attribute entirely when no id is given so the
            // browser doesn't call getElementById("") and emit a console warning.
            r#for: (!props.html_for.is_empty()).then(|| props.html_for.clone()),
            class: format!(
                "text-sm leading-none font-medium select-none peer-disabled:cursor-not-allowed peer-disabled:opacity-50 {}",
                props.class
            ),
            {props.children}
        }
    }
}
