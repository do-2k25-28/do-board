use super::interact;
use crate::pubsub;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use rand::Rng;
use shared::{
    Card, GambleAction, GambleActionRequest, GambleGame, GambleInfo, GambleJoinRequest,
    GambleTableState, Seat, SeatStatus, Slide, SlideConfig, Suit,
};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

const BETTING_WINDOW_SECS: i64 = 20;
const PLAYER_TURN_TIMEOUT_SECS: i64 = 30;
const DEALER_REVEAL_SECS: i64 = 3;
const RESOLVED_DISPLAY_SECS: i64 = 8;

fn status_to_str(status: SeatStatus) -> &'static str {
    match status {
        SeatStatus::Playing => "playing",
        SeatStatus::Stood => "stood",
        SeatStatus::Busted => "busted",
        SeatStatus::Blackjack => "blackjack",
    }
}

fn str_to_status(s: &str) -> SeatStatus {
    match s {
        "stood" => SeatStatus::Stood,
        "busted" => SeatStatus::Busted,
        "blackjack" => SeatStatus::Blackjack,
        _ => SeatStatus::Playing,
    }
}

fn draw_card() -> Card {
    let mut rng = rand::thread_rng();
    let rank = rng.gen_range(1..=13u8);
    let suit = match rng.gen_range(0..4u8) {
        0 => Suit::Hearts,
        1 => Suit::Diamonds,
        2 => Suit::Clubs,
        _ => Suit::Spades,
    };
    Card { rank, suit }
}

/// Best total <=21 if achievable by counting aces as 11 where it helps,
/// otherwise the hard total (aces as 1).
fn hand_value(hand: &[Card]) -> u8 {
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
    total as u8
}

fn is_natural_blackjack(hand: &[Card]) -> bool {
    hand.len() == 2 && hand_value(hand) == 21
}

/// Keeps `gamble_tables` in sync with the Gamble slides currently saved on a
/// screen. Called from `screens::update_screen`, mirroring
/// `interact::sync_sessions`. Only structural fields (screen/game/config) are
/// touched on conflict - an in-progress round's live state is never reset by
/// an unrelated screen edit.
pub async fn sync_tables(
    db: &sqlx::PgPool,
    screen_id: Uuid,
    slides: &[Slide],
) -> Result<(), sqlx::Error> {
    for slide in slides {
        let SlideConfig::Gamble { session_id, game } = &slide.config else {
            continue;
        };
        let Ok(session_uuid) = Uuid::parse_str(session_id) else {
            continue;
        };
        let game_key = match game {
            GambleGame::Blackjack(_) => "blackjack",
        };

        sqlx::query(
            "INSERT INTO gamble_tables (id, screen_id, game, config)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET
                 screen_id = EXCLUDED.screen_id,
                 game = EXCLUDED.game,
                 config = EXCLUDED.config",
        )
        .bind(session_uuid)
        .bind(screen_id)
        .bind(game_key)
        .bind(SqlJson(game))
        .execute(db)
        .await?;
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct TableRow {
    screen_id: Uuid,
    /// The `config` JSONB column - the actual serialized `GambleGame`. Not
    /// to be confused with the `game` column, which is just a plain-text
    /// discriminator ("blackjack") used for filtering, not a JSON value.
    config: SqlJson<GambleGame>,
    phase: String,
    phase_ends_at: chrono::DateTime<chrono::Utc>,
    dealer_hand: SqlJson<Vec<Card>>,
    round_no: i32,
}

#[derive(sqlx::FromRow, Clone)]
struct SeatRow {
    participant_id: Uuid,
    participant_name: String,
    stake: i32,
    hand: SqlJson<Vec<Card>>,
    status: String,
    payout: Option<i32>,
}

async fn fetch_table(db: &sqlx::PgPool, session_id: Uuid) -> Result<Option<TableRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT screen_id, config, phase, phase_ends_at, dealer_hand, round_no
         FROM gamble_tables WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
}

async fn fetch_seats(
    db: &sqlx::PgPool,
    session_id: Uuid,
    round_no: i32,
) -> Result<Vec<SeatRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT participant_id, participant_name, stake, hand, status, payout
         FROM gamble_seats WHERE table_id = $1 AND round_no = $2
         ORDER BY created_at",
    )
    .bind(session_id)
    .bind(round_no)
    .fetch_all(db)
    .await
}

fn to_shared_seats(rows: Vec<SeatRow>) -> Vec<Seat> {
    rows.into_iter()
        .map(|r| Seat {
            participant_id: r.participant_id.to_string(),
            participant_name: r.participant_name,
            stake: r.stake as u32,
            hand: r.hand.0,
            status: str_to_status(&r.status),
            payout: r.payout.map(|p| p as u32),
        })
        .collect()
}

fn build_state(table: &TableRow, seats: Vec<SeatRow>) -> GambleTableState {
    let seats = to_shared_seats(seats);
    match table.phase.as_str() {
        "player_turns" => GambleTableState::PlayerTurns {
            dealer_up_card: table.dealer_hand.0.first().copied().unwrap_or(Card {
                rank: 1,
                suit: Suit::Spades,
            }),
            seats,
        },
        "dealer_play" => GambleTableState::DealerPlay {
            dealer_hand: table.dealer_hand.0.clone(),
            seats,
        },
        "resolved" => GambleTableState::Resolved {
            dealer_hand: table.dealer_hand.0.clone(),
            seats,
        },
        _ => GambleTableState::Betting {
            seats,
            betting_ends_at: table.phase_ends_at.to_rfc3339(),
        },
    }
}

async fn extend_betting_window(db: &sqlx::PgPool, session_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE gamble_tables SET phase_ends_at = NOW() + ($2 || ' seconds')::interval WHERE id = $1",
    )
    .bind(session_id)
    .bind(BETTING_WINDOW_SECS.to_string())
    .execute(db)
    .await?;
    Ok(())
}

async fn deal_initial_hands(
    db: &sqlx::PgPool,
    session_id: Uuid,
    round_no: i32,
    seats: &[SeatRow],
) -> Result<(), sqlx::Error> {
    for seat in seats {
        let hand = vec![draw_card(), draw_card()];
        let status = if is_natural_blackjack(&hand) {
            SeatStatus::Blackjack
        } else {
            SeatStatus::Playing
        };
        sqlx::query(
            "UPDATE gamble_seats SET hand = $1, status = $2
             WHERE table_id = $3 AND round_no = $4 AND participant_id = $5",
        )
        .bind(SqlJson(&hand))
        .bind(status_to_str(status))
        .bind(session_id)
        .bind(round_no)
        .bind(seat.participant_id)
        .execute(db)
        .await?;
    }

    let dealer_hand = vec![draw_card(), draw_card()];
    sqlx::query(
        "UPDATE gamble_tables
         SET dealer_hand = $1, phase = 'player_turns',
             phase_ends_at = NOW() + ($3 || ' seconds')::interval
         WHERE id = $2",
    )
    .bind(SqlJson(&dealer_hand))
    .bind(session_id)
    .bind(PLAYER_TURN_TIMEOUT_SECS.to_string())
    .execute(db)
    .await?;

    Ok(())
}

async fn advance_to_dealer_play(db: &sqlx::PgPool, session_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE gamble_tables SET phase = 'dealer_play', phase_ends_at = NOW() + ($2 || ' seconds')::interval WHERE id = $1",
    )
    .bind(session_id)
    .bind(DEALER_REVEAL_SECS.to_string())
    .execute(db)
    .await?;
    Ok(())
}

async fn resolve_round(
    db: &sqlx::PgPool,
    session_id: Uuid,
    round_no: i32,
    initial_dealer_hand: Vec<Card>,
) -> Result<(), sqlx::Error> {
    let mut dealer_hand = initial_dealer_hand;
    while hand_value(&dealer_hand) < 17 {
        dealer_hand.push(draw_card());
    }
    let dealer_total = hand_value(&dealer_hand);
    let dealer_natural = is_natural_blackjack(&dealer_hand);
    let dealer_bust = dealer_total > 21;

    let seats = fetch_seats(db, session_id, round_no).await?;
    for seat in &seats {
        let stake = seat.stake as u32;
        let payout: u32 = match str_to_status(&seat.status) {
            SeatStatus::Busted => 0,
            SeatStatus::Blackjack => {
                if dealer_natural {
                    stake
                } else {
                    stake + (stake * 3) / 2
                }
            }
            SeatStatus::Playing | SeatStatus::Stood => {
                let player_total = hand_value(&seat.hand.0);
                if dealer_bust || player_total > dealer_total {
                    stake * 2
                } else if player_total == dealer_total {
                    stake
                } else {
                    0
                }
            }
        };
        sqlx::query(
            "UPDATE gamble_seats SET payout = $1
             WHERE table_id = $2 AND round_no = $3 AND participant_id = $4",
        )
        .bind(payout as i32)
        .bind(session_id)
        .bind(round_no)
        .bind(seat.participant_id)
        .execute(db)
        .await?;
    }

    sqlx::query(
        "UPDATE gamble_tables
         SET dealer_hand = $1, phase = 'resolved',
             phase_ends_at = NOW() + ($3 || ' seconds')::interval
         WHERE id = $2",
    )
    .bind(SqlJson(&dealer_hand))
    .bind(session_id)
    .bind(RESOLVED_DISPLAY_SECS.to_string())
    .execute(db)
    .await?;

    Ok(())
}

async fn start_next_round(
    db: &sqlx::PgPool,
    session_id: Uuid,
    round_no: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE gamble_tables
         SET phase = 'betting', phase_ends_at = NOW() + ($3 || ' seconds')::interval,
             dealer_hand = '[]', round_no = $2
         WHERE id = $1",
    )
    .bind(session_id)
    .bind(round_no + 1)
    .bind(BETTING_WINDOW_SECS.to_string())
    .execute(db)
    .await?;
    Ok(())
}

/// Reads a table's state, advancing its phase first if it's due (betting
/// window elapsed, all players done, dealer reveal window elapsed, etc). No
/// background scheduler - every read or action lazily brings the table
/// up to date before returning it. Returns `(game, screen_id, round_no, state)`.
async fn load_or_advance_table(
    db: &sqlx::PgPool,
    session_id: Uuid,
) -> Result<Option<(GambleGame, Uuid, i32, GambleTableState)>, sqlx::Error> {
    let Some(table) = fetch_table(db, session_id).await? else {
        return Ok(None);
    };
    let now = chrono::Utc::now();

    match table.phase.as_str() {
        "betting" if now >= table.phase_ends_at => {
            let seats = fetch_seats(db, session_id, table.round_no).await?;
            if seats.is_empty() {
                extend_betting_window(db, session_id).await?;
            } else {
                deal_initial_hands(db, session_id, table.round_no, &seats).await?;
            }
        }
        "player_turns" => {
            let seats = fetch_seats(db, session_id, table.round_no).await?;
            let all_done = seats.iter().all(|s| s.status != "playing");
            if all_done || now >= table.phase_ends_at {
                advance_to_dealer_play(db, session_id).await?;
            }
        }
        "dealer_play" if now >= table.phase_ends_at => {
            resolve_round(db, session_id, table.round_no, table.dealer_hand.0.clone()).await?;
        }
        "resolved" if now >= table.phase_ends_at => {
            start_next_round(db, session_id, table.round_no).await?;
        }
        _ => {}
    }

    let table = fetch_table(db, session_id)
        .await?
        .expect("table exists - just read it above");
    let seats = fetch_seats(db, session_id, table.round_no).await?;
    let state = build_state(&table, seats);
    Ok(Some((
        table.config.0,
        table.screen_id,
        table.round_no,
        state,
    )))
}

async fn notify_screen(db: &sqlx::PgPool, screen_id: Uuid, session_id: Uuid) {
    let devices: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM devices WHERE current_screen_id = $1 AND online = TRUE")
            .bind(screen_id)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    for device_id in devices {
        let _ = pubsub::notify_gamble_update(db, device_id, session_id).await;
    }
}

/// Fresh `GambleInfo` for a session (with the table's screen id, so callers
/// can push a screen update), or `None` if the table no longer exists.
/// Shared by the public `GET` handler and the pubsub listener - doesn't push
/// notifications itself (the pubsub listener calls this *from* a
/// notification handler; pushing again here would loop).
pub async fn compute_gamble_info(
    db: &sqlx::PgPool,
    session_id: Uuid,
    participant_id: Option<Uuid>,
) -> Result<Option<(GambleInfo, Uuid)>, sqlx::Error> {
    let Some((game, screen_id, _round_no, state)) = load_or_advance_table(db, session_id).await?
    else {
        return Ok(None);
    };

    let balance = match participant_id {
        Some(pid) => Some(interact::compute_participant_balance(db, screen_id, pid).await?),
        None => None,
    };
    let max_stake = match (&state, balance) {
        (GambleTableState::Betting { .. }, Some(balance)) => Some(interact::stake_cap(balance)),
        _ => None,
    };

    Ok(Some((
        GambleInfo {
            game,
            state,
            max_stake,
            balance,
        },
        screen_id,
    )))
}

#[derive(serde::Deserialize)]
pub struct GetTableQuery {
    #[serde(default)]
    participant_id: Option<String>,
}

pub async fn get_table(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<GetTableQuery>,
) -> Result<Json<GambleInfo>, StatusCode> {
    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let participant_id = query
        .participant_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    let (info, screen_id) = compute_gamble_info(&state.db, session_uuid, participant_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    notify_screen(&state.db, screen_id, session_uuid).await;

    Ok(Json(info))
}

pub async fn join_table(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<GambleJoinRequest>,
) -> Result<Json<GambleInfo>, (StatusCode, &'static str)> {
    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID"))?;
    if req.participant_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing participant name"));
    }
    let participant_id = Uuid::parse_str(&req.participant_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid participant ID"))?;

    let (_, screen_id, round_no, table_state) = load_or_advance_table(&state.db, session_uuid)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Table not found"))?;

    if !matches!(table_state, GambleTableState::Betting { .. }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "This round already started - wait for the next one",
        ));
    }

    let balance = interact::compute_participant_balance(&state.db, screen_id, participant_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
    let cap = interact::stake_cap(balance);
    if req.stake < 1 || req.stake > cap {
        return Err((
            StatusCode::BAD_REQUEST,
            "Stake exceeds what you can currently bet",
        ));
    }

    sqlx::query(
        "INSERT INTO gamble_seats (table_id, round_no, participant_id, participant_name, stake)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (table_id, round_no, participant_id)
         DO UPDATE SET stake = EXCLUDED.stake, participant_name = EXCLUDED.participant_name",
    )
    .bind(session_uuid)
    .bind(round_no)
    .bind(participant_id)
    .bind(&req.participant_name)
    .bind(req.stake as i32)
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    notify_screen(&state.db, screen_id, session_uuid).await;

    let (info, _) = compute_gamble_info(&state.db, session_uuid, Some(participant_id))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Table not found"))?;

    Ok(Json(info))
}

pub async fn play_action(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<GambleActionRequest>,
) -> Result<Json<GambleInfo>, (StatusCode, &'static str)> {
    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID"))?;
    let participant_id = Uuid::parse_str(&req.participant_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid participant ID"))?;

    let (_, screen_id, round_no, table_state) = load_or_advance_table(&state.db, session_uuid)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Table not found"))?;

    if !matches!(table_state, GambleTableState::PlayerTurns { .. }) {
        return Err((StatusCode::BAD_REQUEST, "It's not time to play yet"));
    }

    let seat: Option<SeatRow> = sqlx::query_as(
        "SELECT participant_id, participant_name, stake, hand, status, payout
         FROM gamble_seats WHERE table_id = $1 AND round_no = $2 AND participant_id = $3",
    )
    .bind(session_uuid)
    .bind(round_no)
    .bind(participant_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let Some(seat) = seat else {
        return Err((StatusCode::BAD_REQUEST, "You're not seated at this table"));
    };
    if seat.status != "playing" {
        return Err((StatusCode::BAD_REQUEST, "Your hand is already done"));
    }

    match req.action {
        GambleAction::Stand => {
            sqlx::query(
                "UPDATE gamble_seats SET status = 'stood'
                 WHERE table_id = $1 AND round_no = $2 AND participant_id = $3",
            )
            .bind(session_uuid)
            .bind(round_no)
            .bind(participant_id)
            .execute(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
        }
        GambleAction::Hit => {
            let mut hand = seat.hand.0.clone();
            hand.push(draw_card());
            let status = if hand_value(&hand) > 21 {
                "busted"
            } else {
                "playing"
            };
            sqlx::query(
                "UPDATE gamble_seats SET hand = $1, status = $2
                 WHERE table_id = $3 AND round_no = $4 AND participant_id = $5",
            )
            .bind(SqlJson(&hand))
            .bind(status)
            .bind(session_uuid)
            .bind(round_no)
            .bind(participant_id)
            .execute(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
        }
    }

    notify_screen(&state.db, screen_id, session_uuid).await;

    let (info, _) = compute_gamble_info(&state.db, session_uuid, Some(participant_id))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "Table not found"))?;

    Ok(Json(info))
}
