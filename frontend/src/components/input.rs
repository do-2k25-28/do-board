use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct InputProps {
    #[props(default = "text".to_string())]
    pub input_type: String,
    #[props(default)]
    pub placeholder: String,
    #[props(default)]
    pub value: String,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub id: String,
    #[props(default)]
    pub name: String,
    #[props(default)]
    pub class: String,
    #[props(default)]
    pub oninput: Option<EventHandler<String>>,
    #[props(default)]
    pub onchange: Option<EventHandler<String>>,
}

#[component]
pub fn Input(props: InputProps) -> Element {
    rsx! {
        input {
            r#type: props.input_type,
            id: props.id,
            name: props.name,
            placeholder: props.placeholder,
            value: props.value,
            disabled: props.disabled,
            class: format!(
                "border-input flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 {}",
                props.class
            ),
            oninput: move |e: Event<FormData>| {
                if let Some(handler) = &props.oninput {
                    handler.call(e.data.value());
                }
            },
            onchange: move |e: Event<FormData>| {
                if let Some(handler) = &props.onchange {
                    handler.call(e.data.value());
                }
            },
        }
    }
}
