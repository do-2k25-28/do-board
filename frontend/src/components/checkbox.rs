use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct CheckboxProps {
    #[props(default)]
    pub checked: bool,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub id: String,
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub onchange: Option<EventHandler<bool>>,
}

#[component]
pub fn Checkbox(props: CheckboxProps) -> Element {
    let state = if props.checked {
        "checked"
    } else {
        "unchecked"
    };

    rsx! {
        button {
            role: "checkbox",
            id: props.id,
            disabled: props.disabled,
            "aria-checked": "{props.checked}",
            "data-state": state,
            class: format!(
                "peer h-4 w-4 shrink-0 rounded-sm border border-primary shadow-xs outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground {}",
                props.class
            ),
            onclick: move |_| {
                if let Some(handler) = &props.onchange {
                    handler.call(!props.checked);
                }
            },
            if props.checked {
                svg {
                    class: "h-3.5 w-3.5",
                    xmlns: "http://www.w3.org/2000/svg",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2.5",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    polyline { points: "20 6 9 17 4 12" }
                }
            }
        }
    }
}
