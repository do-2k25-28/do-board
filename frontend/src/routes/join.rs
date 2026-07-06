use crate::components::{Button, Input};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use gloo_net::http::Request;
use shared::{
    DrawingStroke, InteractionInfo, InteractionKind, InteractionResponse, InteractionSubmission,
};
use wasm_bindgen::JsCast;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

const CANVAS_SIZE: f64 = 320.0;
const PALETTE: [&str; 6] = [
    "#1f2937", "#ef4444", "#3b82f6", "#22c55e", "#eab308", "#a855f7",
];

async fn submit_response(
    session_id: &str,
    participant_name: String,
    response: InteractionResponse,
) -> bool {
    let result = Request::post(&format!("{API_BASE}/api/interact/{session_id}/respond"))
        .json(&InteractionSubmission {
            participant_name,
            response,
        })
        .unwrap()
        .send()
        .await;
    matches!(result, Ok(r) if r.ok())
}

#[component]
pub fn Join(session_id: String) -> Element {
    let mut info: Signal<Option<InteractionInfo>> = use_signal(|| None);
    let mut load_error = use_signal(|| false);
    let mut participant_name: Signal<Option<String>> = use_signal(|| None);
    let mut name_input = use_signal(String::new);

    {
        let session_id = session_id.clone();
        use_effect(move || {
            let session_id = session_id.clone();
            spawn(async move {
                match Request::get(&format!("{API_BASE}/api/interact/{session_id}"))
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => match resp.json::<InteractionInfo>().await {
                        Ok(parsed) => info.set(Some(parsed)),
                        Err(_) => load_error.set(true),
                    },
                    _ => load_error.set(true),
                }
            });
        });
    }

    rsx! {
        div { class: "min-h-screen flex items-center justify-center p-6 bg-background",
            div { class: "w-full max-w-sm flex flex-col gap-4",
                if load_error() {
                    p { class: "text-center text-sm text-muted-foreground",
                        "This link is no longer active."
                    }
                } else if info().is_none() {
                    p { class: "text-center text-sm text-muted-foreground", "Loading…" }
                } else if participant_name().is_none() {
                    div { class: "flex flex-col gap-3",
                        p { class: "text-sm font-medium", "What's your name?" }
                        Input {
                            placeholder: "Your name",
                            value: name_input(),
                            oninput: move |v| name_input.set(v),
                        }
                        Button {
                            disabled: name_input().trim().is_empty(),
                            onclick: move |_| participant_name.set(Some(name_input().trim().to_string())),
                            "Continue"
                        }
                    }
                } else {
                    {
                        let name = participant_name().unwrap();
                        match info().unwrap().interaction {
                            InteractionKind::Poll { question, options } => rsx! {
                                PollView {
                                    session_id: session_id.clone(),
                                    participant_name: name,
                                    question,
                                    options,
                                }
                            },
                            InteractionKind::Bet { question, options } => rsx! {
                                BetView {
                                    session_id: session_id.clone(),
                                    participant_name: name,
                                    question,
                                    options,
                                }
                            },
                            InteractionKind::Drawing { prompt } => rsx! {
                                DrawingView {
                                    session_id: session_id.clone(),
                                    participant_name: name,
                                    prompt,
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PollView(
    session_id: String,
    participant_name: String,
    question: String,
    options: Vec<String>,
) -> Element {
    let mut submitted = use_signal(|| false);
    let mut submit_error: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        if submitted() {
            p { class: "text-center text-lg font-medium", "Thanks for participating!" }
        } else {
            div { class: "flex flex-col gap-3",
                p { class: "text-lg font-semibold text-center", "{question}" }
                for (i , option) in options.into_iter().enumerate() {
                    {
                        let session_id = session_id.clone();
                        let participant_name = participant_name.clone();
                        rsx! {
                            button {
                                r#type: "button",
                                class: "w-full rounded-lg border border-border px-4 py-3 text-left hover:bg-accent transition-colors",
                                onclick: move |_| {
                                    let session_id = session_id.clone();
                                    let participant_name = participant_name.clone();
                                    spawn(async move {
                                        submit_error.set(None);
                                        let ok = submit_response(
                                                &session_id,
                                                participant_name,
                                                InteractionResponse::Poll { option_index: i },
                                            )
                                            .await;
                                        if ok {
                                            submitted.set(true);
                                        } else {
                                            submit_error.set(Some("Failed to submit, please retry.".into()));
                                        }
                                    });
                                },
                                "{option}"
                            }
                        }
                    }
                }
                if let Some(err) = submit_error() {
                    p { class: "text-xs text-destructive text-center", "{err}" }
                }
            }
        }
    }
}

#[component]
fn BetView(
    session_id: String,
    participant_name: String,
    question: String,
    options: Vec<String>,
) -> Element {
    let mut selected: Signal<Option<usize>> = use_signal(|| None);
    let mut stake = use_signal(|| 10u32);
    let mut submitted = use_signal(|| false);
    let mut submit_error: Signal<Option<String>> = use_signal(|| None);

    let place_bet = move |_| {
        let Some(option_index) = selected() else {
            return;
        };
        let session_id = session_id.clone();
        let participant_name = participant_name.clone();
        let stake_value = stake();
        spawn(async move {
            submit_error.set(None);
            let ok = submit_response(
                &session_id,
                participant_name,
                InteractionResponse::Bet {
                    option_index,
                    stake: stake_value,
                },
            )
            .await;
            if ok {
                submitted.set(true);
            } else {
                submit_error.set(Some("Failed to submit, please retry.".into()));
            }
        });
    };

    rsx! {
        if submitted() {
            p { class: "text-center text-lg font-medium", "Thanks for participating!" }
        } else {
            div { class: "flex flex-col gap-3",
                p { class: "text-lg font-semibold text-center", "{question}" }
                for (i , option) in options.iter().enumerate() {
                    {
                        let is_selected = selected() == Some(i);
                        rsx! {
                            button {
                                r#type: "button",
                                class: if is_selected {
                                    "w-full rounded-lg border-2 border-ring bg-accent px-4 py-3 text-left font-medium transition-colors"
                                } else {
                                    "w-full rounded-lg border border-border px-4 py-3 text-left hover:bg-accent transition-colors"
                                },
                                onclick: move |_| selected.set(Some(i)),
                                "{option}"
                            }
                        }
                    }
                }
                div { class: "flex items-center gap-2",
                    span { class: "text-sm", "Stake:" }
                    input {
                        r#type: "number",
                        min: "1",
                        max: "1000",
                        class: "border-input flex h-9 w-24 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
                        value: "{stake}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                stake.set(v.clamp(1, 1000));
                            }
                        },
                    }
                    span { class: "text-sm text-muted-foreground", "points" }
                }
                Button {
                    disabled: selected().is_none(),
                    onclick: place_bet,
                    "Place bet"
                }
                if let Some(err) = submit_error() {
                    p { class: "text-xs text-destructive text-center", "{err}" }
                }
            }
        }
    }
}

#[component]
fn DrawingView(session_id: String, participant_name: String, prompt: String) -> Element {
    let mut canvas_el: Signal<Option<web_sys::HtmlCanvasElement>> = use_signal(|| None);
    let mut current_color = use_signal(|| PALETTE[0].to_string());
    let mut current_stroke: Signal<Vec<[f32; 2]>> = use_signal(Vec::new);
    let mut is_drawing = use_signal(|| false);

    let get_ctx = move || -> Option<web_sys::CanvasRenderingContext2d> {
        canvas_el()?
            .get_context("2d")
            .ok()??
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .ok()
    };

    let mut finish_stroke = move || {
        if !is_drawing() {
            return;
        }
        is_drawing.set(false);
        let points = current_stroke();
        current_stroke.set(vec![]);
        if points.is_empty() {
            return;
        }
        let session_id = session_id.clone();
        let participant_name = participant_name.clone();
        let color = current_color();
        spawn(async move {
            let _ = submit_response(
                &session_id,
                participant_name,
                InteractionResponse::Drawing {
                    stroke: DrawingStroke { color, points },
                },
            )
            .await;
        });
    };

    rsx! {
        div { class: "flex flex-col items-center gap-4",
            p { class: "text-lg font-semibold text-center", "{prompt}" }
            div { class: "flex gap-2",
                for color in PALETTE {
                    button {
                        r#type: "button",
                        class: "size-7 rounded-full border-2",
                        style: if current_color() == color {
                            format!("background-color:{color};border-color:#111827;")
                        } else {
                            format!("background-color:{color};border-color:transparent;")
                        },
                        onclick: move |_| current_color.set(color.to_string()),
                    }
                }
            }
            canvas {
                width: "320",
                height: "320",
                class: "rounded-lg border border-border bg-white touch-none",
                style: "width:{CANVAS_SIZE}px;height:{CANVAS_SIZE}px;",
                onmounted: move |e| {
                    let web_el = e.data().as_web_event();
                    if let Ok(canvas) = web_el.dyn_into::<web_sys::HtmlCanvasElement>() {
                        canvas_el.set(Some(canvas));
                    }
                },
                onpointerdown: move |e| {
                    let p = e.element_coordinates();
                    let x = (p.x / CANVAS_SIZE).clamp(0.0, 1.0) as f32;
                    let y = (p.y / CANVAS_SIZE).clamp(0.0, 1.0) as f32;
                    current_stroke.set(vec![[x, y]]);
                    is_drawing.set(true);
                    if let Some(ctx) = get_ctx() {
                        ctx.set_stroke_style_str(&current_color());
                        ctx.set_line_width(4.0);
                        ctx.set_line_cap("round");
                        ctx.set_line_join("round");
                        ctx.begin_path();
                        ctx.move_to(p.x, p.y);
                    }
                },
                onpointermove: move |e| {
                    if !is_drawing() {
                        return;
                    }
                    let p = e.element_coordinates();
                    let x = (p.x / CANVAS_SIZE).clamp(0.0, 1.0) as f32;
                    let y = (p.y / CANVAS_SIZE).clamp(0.0, 1.0) as f32;
                    current_stroke.write().push([x, y]);
                    if let Some(ctx) = get_ctx() {
                        ctx.line_to(p.x, p.y);
                        ctx.stroke();
                    }
                },
                onpointerup: {
                    let mut finish_stroke = finish_stroke.clone();
                    move |_| finish_stroke()
                },
                onpointerleave: move |_| finish_stroke(),
            }
            p { class: "text-xs text-muted-foreground text-center",
                "Draw as many strokes as you like - everyone shares the same canvas."
            }
        }
    }
}
