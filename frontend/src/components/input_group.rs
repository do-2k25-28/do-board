use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct InputGroupProps {
    #[props(default)]
    pub left_addon: Option<Element>,
    #[props(default)]
    pub right_addon: Option<Element>,
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn InputGroup(props: InputGroupProps) -> Element {
    rsx! {
        div {
            class: format!("relative flex items-center {}", props.class),
            if let Some(left) = props.left_addon {
                div {
                    class: "absolute left-3 flex items-center pointer-events-none text-muted-foreground",
                    {left}
                }
            }
            {props.children}
            if let Some(right) = props.right_addon {
                div {
                    class: "absolute right-3 flex items-center pointer-events-none text-muted-foreground",
                    {right}
                }
            }
        }
    }
}
