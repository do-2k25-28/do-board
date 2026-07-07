use crate::components::{Button, ButtonVariant, Input};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use shared::{
    BetMarket, BetOutcome, Card, DrawingStroke, GambleAction, GambleActionRequest, GambleInfo,
    GambleJoinRequest, GambleTableState, InteractionInfo, InteractionKind, InteractionResponse,
    InteractionResults, InteractionSubmission, SeatStatus, Suit,
};
use uuid::Uuid;
use wasm_bindgen::JsCast;

const API_BASE: &str = match option_env!("API_BASE") {
    Some(v) => v,
    None => "",
};

const CANVAS_SIZE: f64 = 320.0;
const PALETTE: [&str; 6] = [
    "#1f2937", "#ef4444", "#3b82f6", "#22c55e", "#eab308", "#a855f7",
];

const PARTICIPANT_ID_KEY: &str = "do-board:participant-id";
const PARTICIPANT_NAME_KEY: &str = "do-board:participant-name";

/// Stable id for this browser, generated once and persisted in
/// `localStorage`, so a participant's bets can be tracked across multiple
/// sessions on the same screen (leaderboard).
fn participant_id() -> String {
    let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
    if let Some(storage) = &storage {
        if let Ok(Some(existing)) = storage.get_item(PARTICIPANT_ID_KEY) {
            return existing;
        }
    }
    let id = Uuid::new_v4().to_string();
    if let Some(storage) = &storage {
        let _ = storage.set_item(PARTICIPANT_ID_KEY, &id);
    }
    id
}

/// The name typed on a previous visit, if any, so returning participants
/// aren't asked for it again on every new join link.
fn stored_participant_name() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(PARTICIPANT_NAME_KEY).ok().flatten())
        .filter(|name: &String| !name.trim().is_empty())
}

fn save_participant_name(name: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(PARTICIPANT_NAME_KEY, name);
    }
}

fn bet_submitted_key(session_id: &str) -> String {
    format!("do-board:bet-submitted:{session_id}")
}

/// Whether this browser has already placed a bet for this session - the
/// server is the authority (it rejects a second bet per participant), this
/// is just so the form doesn't even show up again on a reload/revisit.
fn already_submitted_bet(session_id: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|storage| {
            storage
                .get_item(&bet_submitted_key(session_id))
                .ok()
                .flatten()
        })
        .is_some()
}

fn mark_bet_submitted(session_id: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(&bet_submitted_key(session_id), "1");
    }
}

async fn submit_response(
    session_id: &str,
    participant_name: String,
    response: InteractionResponse,
) -> Result<(), String> {
    let result = Request::post(&format!("{API_BASE}/api/interact/{session_id}/respond"))
        .json(&InteractionSubmission {
            participant_name,
            participant_id: participant_id(),
            response,
        })
        .unwrap()
        .send()
        .await;
    match result {
        Ok(r) if r.ok() => Ok(()),
        Ok(r) => Err(r
            .text()
            .await
            .unwrap_or_else(|_| "Failed to submit, please retry.".into())),
        Err(_) => Err("Failed to submit, please retry.".into()),
    }
}

/// Small "Your score: N pts" line, shared by `BetView` and `GambleView` - the
/// same net balance (payouts minus stakes, across bets and gamble) that
/// `stake_cap` is derived from, so it can read negative.
fn score_badge(balance: Option<i64>) -> Element {
    let Some(balance) = balance else {
        return rsx! {};
    };
    let class = if balance > 0 {
        "text-green-600"
    } else if balance < 0 {
        "text-destructive"
    } else {
        "text-muted-foreground"
    };
    rsx! {
        p { class: "text-xs text-center {class}", "Your score: {balance} pts" }
    }
}

#[component]
pub fn Join(session_id: String) -> Element {
    let mut info: Signal<Option<InteractionInfo>> = use_signal(|| None);
    let mut load_error = use_signal(|| false);
    let mut participant_name: Signal<Option<String>> = use_signal(stored_participant_name);
    let mut name_input = use_signal(String::new);

    {
        let session_id = session_id.clone();
        use_effect(move || {
            let session_id = session_id.clone();
            spawn(async move {
                let pid = participant_id();
                match Request::get(&format!(
                    "{API_BASE}/api/interact/{session_id}?participant_id={pid}"
                ))
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
                            onclick: move |_| {
                                let name = name_input().trim().to_string();
                                save_participant_name(&name);
                                participant_name.set(Some(name));
                            },
                            "Continue"
                        }
                    }
                } else {
                    {
                        let name = participant_name().unwrap();
                        let info_value = info().unwrap();
                        let max_stake = info_value.max_stake;
                        let balance = info_value.balance;
                        match info_value.interaction {
                            InteractionKind::Poll { question, options } => rsx! {
                                PollView {
                                    session_id: session_id.clone(),
                                    participant_name: name,
                                    question,
                                    options,
                                }
                            },
                            InteractionKind::Bet {
                                question,
                                market,
                                result,
                            } => rsx! {
                                BetView {
                                    session_id: session_id.clone(),
                                    participant_name: name,
                                    question,
                                    market,
                                    result,
                                    max_stake,
                                    balance,
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

/// Join page for a `Gamble` (Blackjack) table - separate from `Join` since a
/// gamble table isn't an `InteractionKind`/`InteractionInfo` at all, it's a
/// continuously evolving game state fetched from `/api/gamble/{session_id}`.
#[component]
pub fn GambleJoin(session_id: String) -> Element {
    let mut participant_name: Signal<Option<String>> = use_signal(stored_participant_name);
    let mut name_input = use_signal(String::new);

    rsx! {
        div { class: "min-h-screen flex items-center justify-center p-6 bg-background",
            div { class: "w-full max-w-sm flex flex-col gap-4",
                if participant_name().is_none() {
                    div { class: "flex flex-col gap-3",
                        p { class: "text-sm font-medium", "What's your name?" }
                        Input {
                            placeholder: "Your name",
                            value: name_input(),
                            oninput: move |v| name_input.set(v),
                        }
                        Button {
                            disabled: name_input().trim().is_empty(),
                            onclick: move |_| {
                                let name = name_input().trim().to_string();
                                save_participant_name(&name);
                                participant_name.set(Some(name));
                            },
                            "Continue"
                        }
                    }
                } else {
                    GambleView {
                        session_id: session_id.clone(),
                        participant_name: participant_name().unwrap(),
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
                                        let result = submit_response(
                                            &session_id,
                                            participant_name,
                                            InteractionResponse::Poll { option_index: i },
                                        )
                                        .await;
                                        match result {
                                            Ok(()) => submitted.set(true),
                                            Err(msg) => submit_error.set(Some(msg)),
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

fn format_bet_outcome(market: &BetMarket, outcome: &BetOutcome) -> String {
    match (market, outcome) {
        (BetMarket::Options { options }, BetOutcome::Options { option_index }) => options
            .get(*option_index)
            .cloned()
            .unwrap_or_else(|| format!("Option {option_index}")),
        (
            BetMarket::Score {
                home_label,
                away_label,
            },
            BetOutcome::Score {
                home_score,
                away_score,
            },
        ) => format!("{home_label} {home_score} - {away_score} {away_label}"),
        (
            _,
            BetOutcome::Score {
                home_score,
                away_score,
            },
        ) => format!("{home_score} - {away_score}"),
        (_, BetOutcome::Options { option_index }) => format!("Option {option_index}"),
    }
}

/// Fallback shown while the server-computed cap hasn't loaded yet. The
/// server (not this default) is what actually enforces the limit.
const DEFAULT_STAKE_CAP: u32 = 10;

#[component]
fn BetView(
    session_id: String,
    participant_name: String,
    question: String,
    market: BetMarket,
    result: Option<BetOutcome>,
    max_stake: Option<u32>,
    balance: Option<i64>,
) -> Element {
    let stake_cap = max_stake.unwrap_or(DEFAULT_STAKE_CAP).max(1);
    let mut selected: Signal<Option<usize>> = use_signal(|| None);
    let mut home_score = use_signal(|| 0u32);
    let mut away_score = use_signal(|| 0u32);
    let mut stake = use_signal(move || stake_cap.min(DEFAULT_STAKE_CAP));
    let mut submitted = use_signal({
        let session_id = session_id.clone();
        move || already_submitted_bet(&session_id)
    });
    let mut submit_error: Signal<Option<String>> = use_signal(|| None);

    if let Some(result) = &result {
        return rsx! {
            div { class: "flex flex-col gap-2 items-center",
                p { class: "text-lg font-semibold text-center", "{question}" }
                p { class: "text-sm text-muted-foreground text-center", "Betting is closed." }
                p { class: "text-base font-medium text-center",
                    "Result: {format_bet_outcome(&market, result)}"
                }
                {score_badge(balance)}
            }
        };
    }

    let bet_market_for_submit = market.clone();
    let place_bet = move |_| {
        let pick = match &bet_market_for_submit {
            BetMarket::Options { .. } => {
                let Some(option_index) = selected() else {
                    return;
                };
                BetOutcome::Options { option_index }
            }
            BetMarket::Score { .. } => BetOutcome::Score {
                home_score: home_score(),
                away_score: away_score(),
            },
        };
        let session_id = session_id.clone();
        let participant_name = participant_name.clone();
        let stake_value = stake();
        spawn(async move {
            submit_error.set(None);
            let result = submit_response(
                &session_id,
                participant_name,
                InteractionResponse::Bet {
                    pick,
                    stake: stake_value,
                },
            )
            .await;
            match result {
                Ok(()) => {
                    mark_bet_submitted(&session_id);
                    submitted.set(true);
                }
                Err(msg) => submit_error.set(Some(msg)),
            }
        });
    };

    let can_submit = match &market {
        BetMarket::Options { .. } => selected().is_some(),
        BetMarket::Score { .. } => true,
    };

    rsx! {
        if submitted() {
            p { class: "text-center text-lg font-medium", "Thanks for participating!" }
        } else {
            div { class: "flex flex-col gap-3",
                p { class: "text-lg font-semibold text-center", "{question}" }
                match &market {
                    BetMarket::Options { options } => rsx! {
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
                    },
                    BetMarket::Score { home_label, away_label } => rsx! {
                        div { class: "flex items-center justify-center gap-3",
                            div { class: "flex flex-col items-center gap-1",
                                span { class: "text-sm font-medium", "{home_label}" }
                                input {
                                    r#type: "number",
                                    min: "0",
                                    max: "99",
                                    class: "border-input flex h-11 w-16 rounded-md border bg-transparent px-2 py-1 text-center text-lg shadow-xs outline-none",
                                    value: "{home_score}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<u32>() {
                                            home_score.set(v.clamp(0, 99));
                                        }
                                    },
                                }
                            }
                            span { class: "text-lg font-semibold", "-" }
                            div { class: "flex flex-col items-center gap-1",
                                span { class: "text-sm font-medium", "{away_label}" }
                                input {
                                    r#type: "number",
                                    min: "0",
                                    max: "99",
                                    class: "border-input flex h-11 w-16 rounded-md border bg-transparent px-2 py-1 text-center text-lg shadow-xs outline-none",
                                    value: "{away_score}",
                                    oninput: move |e| {
                                        if let Ok(v) = e.value().parse::<u32>() {
                                            away_score.set(v.clamp(0, 99));
                                        }
                                    },
                                }
                            }
                        }
                    },
                }
                div { class: "flex items-center gap-2",
                    span { class: "text-sm", "Stake:" }
                    input {
                        r#type: "number",
                        min: "1",
                        max: "{stake_cap}",
                        class: "border-input flex h-9 w-24 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
                        value: "{stake}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                stake.set(v.clamp(1, stake_cap));
                            }
                        },
                    }
                    span { class: "text-sm text-muted-foreground", "points" }
                }
                p { class: "text-xs text-muted-foreground text-center",
                    "You can bet up to {stake_cap} points."
                }
                {score_badge(balance)}
                Button {
                    disabled: !can_submit,
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
    let mut shared_strokes: Signal<Vec<DrawingStroke>> = use_signal(Vec::new);

    // Poll the shared results so this phone sees every participant's strokes,
    // not just its own - a phone is never a registered "device", so it can't
    // receive the WebSocket pushes the display screen gets.
    use_coroutine({
        let session_id = session_id.clone();
        move |_: UnboundedReceiver<()>| {
            let session_id = session_id.clone();
            async move {
                loop {
                    if let Ok(resp) = Request::get(&format!("{API_BASE}/api/interact/{session_id}"))
                        .send()
                        .await
                    {
                        if resp.ok() {
                            if let Ok(info) = resp.json::<InteractionInfo>().await {
                                if let InteractionResults::Drawing { strokes } = info.results {
                                    shared_strokes.set(strokes);
                                }
                            }
                        }
                    }
                    TimeoutFuture::new(1200).await;
                }
            }
        }
    });

    let get_ctx = move || -> Option<web_sys::CanvasRenderingContext2d> {
        canvas_el()?
            .get_context("2d")
            .ok()??
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .ok()
    };

    // Redraw the full shared canvas whenever the polled strokes change. This
    // is skipped mid-stroke so it doesn't clobber the line the participant is
    // currently drawing; it fires again as soon as that stroke finishes.
    use_effect(move || {
        let strokes = shared_strokes();
        if is_drawing() {
            return;
        }
        let Some(ctx) = get_ctx() else {
            return;
        };
        ctx.clear_rect(0.0, 0.0, CANVAS_SIZE, CANVAS_SIZE);
        ctx.set_line_width(4.0);
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        for stroke in &strokes {
            let mut points = stroke.points.iter();
            let Some(first) = points.next() else {
                continue;
            };
            ctx.set_stroke_style_str(&stroke.color);
            ctx.begin_path();
            ctx.move_to(first[0] as f64 * CANVAS_SIZE, first[1] as f64 * CANVAS_SIZE);
            for p in points {
                ctx.line_to(p[0] as f64 * CANVAS_SIZE, p[1] as f64 * CANVAS_SIZE);
            }
            ctx.stroke();
        }
    });

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
        let stroke = DrawingStroke { color, points };
        // Optimistic: show our own stroke immediately instead of waiting for
        // the next poll round-trip to echo it back.
        shared_strokes.write().push(stroke.clone());
        spawn(async move {
            let _ = submit_response(
                &session_id,
                participant_name,
                InteractionResponse::Drawing { stroke },
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

// ── Gamble (Blackjack) ───────────────────────────────────────────────────────

fn card_label(card: &Card) -> String {
    let rank = match card.rank {
        1 => "A".to_string(),
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        n => n.to_string(),
    };
    let suit = match card.suit {
        Suit::Hearts => "♥",
        Suit::Diamonds => "♦",
        Suit::Clubs => "♣",
        Suit::Spades => "♠",
    };
    format!("{rank}{suit}")
}

fn hand_label(hand: &[Card]) -> String {
    hand.iter().map(card_label).collect::<Vec<_>>().join(" ")
}

/// Mirrors the backend's ace-aware hand value purely for display - the
/// server always has the last word on busts/blackjacks/payouts.
fn hand_total_label(hand: &[Card]) -> String {
    let mut total: u16 = 0;
    let mut aces = 0u8;
    for card in hand {
        total += u16::from(card.rank.min(10));
        if card.rank == 1 {
            aces += 1;
        }
    }
    while aces > 0 && total + 10 <= 21 {
        total += 10;
        aces -= 1;
    }
    total.to_string()
}

fn seat_status_message(status: SeatStatus) -> &'static str {
    match status {
        SeatStatus::Playing => "Your turn.",
        SeatStatus::Stood => "You stood - waiting for the others.",
        SeatStatus::Busted => "Busted - waiting for the round to finish.",
        SeatStatus::Blackjack => "Blackjack! Waiting for the round to finish.",
    }
}

async fn gamble_join(session_id: &str, participant_name: String, stake: u32) -> Result<(), String> {
    let result = Request::post(&format!("{API_BASE}/api/gamble/{session_id}/join"))
        .json(&GambleJoinRequest {
            participant_id: participant_id(),
            participant_name,
            stake,
        })
        .unwrap()
        .send()
        .await;
    match result {
        Ok(r) if r.ok() => Ok(()),
        Ok(r) => Err(r
            .text()
            .await
            .unwrap_or_else(|_| "Failed to join, please retry.".into())),
        Err(_) => Err("Failed to join, please retry.".into()),
    }
}

async fn gamble_action(session_id: &str, action: GambleAction) -> Result<(), String> {
    let result = Request::post(&format!("{API_BASE}/api/gamble/{session_id}/action"))
        .json(&GambleActionRequest {
            participant_id: participant_id(),
            action,
        })
        .unwrap()
        .send()
        .await;
    match result {
        Ok(r) if r.ok() => Ok(()),
        Ok(r) => Err(r
            .text()
            .await
            .unwrap_or_else(|_| "Failed to act, please retry.".into())),
        Err(_) => Err("Failed to act, please retry.".into()),
    }
}

#[component]
fn GambleView(session_id: String, participant_name: String) -> Element {
    let mut table: Signal<Option<GambleInfo>> = use_signal(|| None);
    let mut load_error = use_signal(|| false);
    let mut stake = use_signal(|| DEFAULT_STAKE_CAP);
    let mut joining = use_signal(|| false);
    let mut action_error: Signal<Option<String>> = use_signal(|| None);

    use_coroutine({
        let session_id = session_id.clone();
        move |_: UnboundedReceiver<()>| {
            let session_id = session_id.clone();
            async move {
                loop {
                    let pid = participant_id();
                    match Request::get(&format!(
                        "{API_BASE}/api/gamble/{session_id}?participant_id={pid}"
                    ))
                    .send()
                    .await
                    {
                        Ok(resp) if resp.ok() => {
                            if let Ok(info) = resp.json::<GambleInfo>().await {
                                table.set(Some(info));
                            }
                        }
                        Ok(_) => load_error.set(true),
                        Err(_) => {}
                    }
                    TimeoutFuture::new(1500).await;
                }
            }
        }
    });

    rsx! {
        if load_error() {
            p { class: "text-center text-sm text-muted-foreground", "This table is no longer available." }
        } else if table().is_none() {
            p { class: "text-center text-sm text-muted-foreground", "Loading…" }
        } else {
            {
                let info = table().unwrap();
                let my_seat = match &info.state {
                    GambleTableState::Betting { seats, .. }
                    | GambleTableState::PlayerTurns { seats, .. }
                    | GambleTableState::DealerPlay { seats, .. }
                    | GambleTableState::Resolved { seats, .. } => seats
                        .iter()
                        .find(|s| s.participant_id == participant_id())
                        .cloned(),
                };
                let stake_cap = info.max_stake.unwrap_or(DEFAULT_STAKE_CAP).max(1);

                match &info.state {
                    GambleTableState::Betting { .. } => rsx! {
                        if let Some(seat) = &my_seat {
                            div { class: "flex flex-col items-center gap-2",
                                p { class: "text-lg font-semibold text-center", "You're in for {seat.stake} pts" }
                                p { class: "text-sm text-muted-foreground text-center", "Waiting for the round to start…" }
                            }
                        } else {
                            div { class: "flex flex-col gap-3",
                                p { class: "text-lg font-semibold text-center", "Place your bet" }
                                div { class: "flex items-center gap-2 justify-center",
                                    span { class: "text-sm", "Stake:" }
                                    input {
                                        r#type: "number",
                                        min: "1",
                                        max: "{stake_cap}",
                                        class: "border-input flex h-9 w-24 rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
                                        value: "{stake}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<u32>() {
                                                stake.set(v.clamp(1, stake_cap));
                                            }
                                        },
                                    }
                                }
                                p { class: "text-xs text-muted-foreground text-center",
                                    "You can bet up to {stake_cap} points."
                                }
                                {score_badge(info.balance)}
                                Button {
                                    disabled: joining(),
                                    onclick: {
                                        let session_id = session_id.clone();
                                        let participant_name = participant_name.clone();
                                        move |_| {
                                            let session_id = session_id.clone();
                                            let participant_name = participant_name.clone();
                                            let stake_value = stake().min(stake_cap).max(1);
                                            joining.set(true);
                                            action_error.set(None);
                                            spawn(async move {
                                                let result = gamble_join(&session_id, participant_name, stake_value).await;
                                                joining.set(false);
                                                if let Err(msg) = result {
                                                    action_error.set(Some(msg));
                                                }
                                            });
                                        }
                                    },
                                    "Sit down"
                                }
                                if let Some(err) = action_error() {
                                    p { class: "text-xs text-destructive text-center", "{err}" }
                                }
                            }
                        }
                    },
                    GambleTableState::PlayerTurns { dealer_up_card, .. } => rsx! {
                        match &my_seat {
                            None => rsx! {
                                p { class: "text-sm text-muted-foreground text-center",
                                    "This round already started - you'll be seated for the next one."
                                }
                            },
                            Some(seat) => rsx! {
                                div { class: "flex flex-col items-center gap-4",
                                    div { class: "flex flex-col items-center gap-1",
                                        p { class: "text-xs text-muted-foreground", "Dealer shows" }
                                        p { class: "text-2xl font-semibold", "{card_label(dealer_up_card)}" }
                                    }
                                    div { class: "flex flex-col items-center gap-1",
                                        p { class: "text-xs text-muted-foreground", "Your hand ({seat.stake} pts)" }
                                        p { class: "text-2xl font-semibold", "{hand_label(&seat.hand)}" }
                                        p { class: "text-sm text-muted-foreground", "Total: {hand_total_label(&seat.hand)}" }
                                    }
                                    if seat.status == SeatStatus::Playing {
                                        div { class: "flex gap-2",
                                            Button {
                                                onclick: {
                                                    let session_id = session_id.clone();
                                                    move |_| {
                                                        let session_id = session_id.clone();
                                                        spawn(async move {
                                                            action_error.set(None);
                                                            if let Err(msg) = gamble_action(&session_id, GambleAction::Hit).await {
                                                                action_error.set(Some(msg));
                                                            }
                                                        });
                                                    }
                                                },
                                                "Hit"
                                            }
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                onclick: {
                                                    let session_id = session_id.clone();
                                                    move |_| {
                                                        let session_id = session_id.clone();
                                                        spawn(async move {
                                                            action_error.set(None);
                                                            if let Err(msg) = gamble_action(&session_id, GambleAction::Stand).await {
                                                                action_error.set(Some(msg));
                                                            }
                                                        });
                                                    }
                                                },
                                                "Stand"
                                            }
                                        }
                                    } else {
                                        p { class: "text-sm text-muted-foreground text-center", "{seat_status_message(seat.status)}" }
                                    }
                                    if let Some(err) = action_error() {
                                        p { class: "text-xs text-destructive text-center", "{err}" }
                                    }
                                }
                            },
                        }
                    },
                    GambleTableState::DealerPlay { dealer_hand, .. } => rsx! {
                        div { class: "flex flex-col items-center gap-3",
                            p { class: "text-lg font-semibold text-center", "Dealer is drawing…" }
                            p { class: "text-xl", "{hand_label(dealer_hand)}" }
                            if let Some(seat) = &my_seat {
                                p { class: "text-sm text-muted-foreground text-center",
                                    "Your hand: {hand_label(&seat.hand)} ({hand_total_label(&seat.hand)})"
                                }
                            }
                        }
                    },
                    GambleTableState::Resolved { dealer_hand, .. } => rsx! {
                        div { class: "flex flex-col items-center gap-3",
                            p { class: "text-base text-muted-foreground text-center",
                                "Dealer: {hand_label(dealer_hand)} ({hand_total_label(dealer_hand)})"
                            }
                            if let Some(seat) = &my_seat {
                                {
                                    let payout = seat.payout.unwrap_or(0);
                                    let net = payout as i64 - seat.stake as i64;
                                    let (msg, class) = if net > 0 {
                                        (format!("You won {net} pts!"), "text-green-600")
                                    } else if net == 0 {
                                        ("Push - your stake is back.".to_string(), "text-muted-foreground")
                                    } else {
                                        ("You lost your stake.".to_string(), "text-destructive")
                                    };
                                    rsx! {
                                        p { class: "text-lg font-semibold text-center {class}", "{msg}" }
                                        {score_badge(info.balance)}
                                    }
                                }
                            } else {
                                p { class: "text-sm text-muted-foreground text-center", "Next round starting soon…" }
                            }
                        }
                    },
                }
            }
        }
    }
}
