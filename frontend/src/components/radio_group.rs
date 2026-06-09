use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct RadioGroupProps {
    #[props(default)]
    pub class: String,
    pub children: Element,
}

#[component]
pub fn RadioGroup(props: RadioGroupProps) -> Element {
    rsx! {
        div {
            role: "radiogroup",
            class: format!("grid gap-3 {}", props.class),
            {props.children}
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct RadioGroupItemProps {
    pub value: String,
    #[props(default)]
    pub checked: bool,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub id: String,
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub onchange: Option<EventHandler<String>>,
}

#[component]
pub fn RadioGroupItem(props: RadioGroupItemProps) -> Element {
    let state = if props.checked {
        "checked"
    } else {
        "unchecked"
    };
    let value = props.value.clone();

    rsx! {
        button {
            role: "radio",
            id: props.id,
            disabled: props.disabled,
            "aria-checked": "{props.checked}",
            "data-state": state,
            class: format!(
                "aspect-square h-4 w-4 rounded-full border border-primary shadow-xs outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 {}",
                props.class
            ),
            onclick: move |_| {
                if let Some(handler) = &props.onchange {
                    handler.call(value.clone());
                }
            },
            if props.checked {
                div { class: "flex items-center justify-center",
                    div { class: "h-2 w-2 rounded-full bg-primary" }
                }
            }
        }
    }
}
