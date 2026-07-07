use crate::pubsub;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use shared::{
    BetMarket, BetOutcome, BetOutcomeTotal, BetPayout, DrawingStroke, InteractionInfo,
    InteractionKind, InteractionResponse, InteractionResults, InteractionSubmission,
    LeaderboardEntry, Slide, SlideConfig,
};
use sqlx::types::Json as SqlJson;
use std::collections::HashMap;
use uuid::Uuid;

const MAX_STAKE: u32 = 1000;
const MAX_SCORE: u32 = 99;
const MAX_STROKE_POINTS: usize = 2000;
/// Stake ceiling for a participant with no (or a non-positive) net balance
/// yet, so nobody who hasn't won anything is locked out of betting. Once a
/// participant has a positive balance, they're capped at exactly that -
/// never above what they've actually won.
const FREE_STAKE_CAP: u32 = 10;

/// The most a participant may stake given their current net balance (payouts
/// won minus stakes spent, across every resolved bet they've taken on this
/// screen).
pub(crate) fn stake_cap(balance: i64) -> u32 {
    if balance > 0 {
        u32::try_from(balance).unwrap_or(MAX_STAKE).min(MAX_STAKE)
    } else {
        FREE_STAKE_CAP
    }
}

fn kind_key(kind: &InteractionKind) -> &'static str {
    match kind {
        InteractionKind::Poll { .. } => "poll",
        InteractionKind::Bet { .. } => "bet",
        InteractionKind::Drawing { .. } => "drawing",
    }
}

/// Identifies one possible outcome of a `Bet`, independent of which market it
/// came from - used to group responses and compute pari-mutuel odds.
#[derive(Clone, PartialEq, Eq, Hash)]
enum OutcomeKey {
    Options(usize),
    Score(u32, u32),
}

impl OutcomeKey {
    fn from_outcome(outcome: &BetOutcome) -> Self {
        match outcome {
            BetOutcome::Options { option_index } => OutcomeKey::Options(*option_index),
            BetOutcome::Score {
                home_score,
                away_score,
            } => OutcomeKey::Score(*home_score, *away_score),
        }
    }

    fn label(&self, market: &BetMarket) -> String {
        match self {
            OutcomeKey::Options(i) => match market {
                BetMarket::Options { options } => options
                    .get(*i)
                    .cloned()
                    .unwrap_or_else(|| format!("Option {i}")),
                BetMarket::Score { .. } => format!("Option {i}"),
            },
            OutcomeKey::Score(home, away) => format!("{home}-{away}"),
        }
    }
}

/// Keeps `interaction_sessions` in sync with the interactive slides currently
/// saved on a screen. Called from `screens::update_screen` right after a
/// screen's slides are persisted.
pub async fn sync_sessions(
    db: &sqlx::PgPool,
    screen_id: Uuid,
    slides: &[Slide],
) -> Result<(), sqlx::Error> {
    for slide in slides {
        let SlideConfig::Interactive {
            session_id,
            interaction,
        } = &slide.config
        else {
            continue;
        };
        let Ok(session_uuid) = Uuid::parse_str(session_id) else {
            continue;
        };

        sqlx::query(
            "INSERT INTO interaction_sessions (id, screen_id, kind, config, updated_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (id) DO UPDATE SET
                 screen_id = EXCLUDED.screen_id,
                 kind = EXCLUDED.kind,
                 config = EXCLUDED.config,
                 updated_at = NOW()",
        )
        .bind(session_uuid)
        .bind(screen_id)
        .bind(kind_key(interaction))
        .bind(SqlJson(interaction))
        .execute(db)
        .await?;
    }

    Ok(())
}

/// Session ids for every interactive slide on a screen - used after a screen
/// save to push fresh results to any device currently displaying it (e.g. so
/// a newly-entered bet result shows payouts immediately).
pub fn interactive_session_ids(slides: &[Slide]) -> Vec<Uuid> {
    slides
        .iter()
        .filter_map(|slide| match &slide.config {
            SlideConfig::Interactive { session_id, .. } => Uuid::parse_str(session_id).ok(),
            _ => None,
        })
        .collect()
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    screen_id: Uuid,
    config: SqlJson<InteractionKind>,
}

#[derive(sqlx::FromRow)]
struct BetResponseRow {
    participant_id: Uuid,
    participant_name: String,
    payload: SqlJson<InteractionResponse>,
}

/// One participant's response to a `Bet`, with their payout resolved (0 and
/// `is_winner: false` while the bet is still open or if they didn't pick the
/// winning outcome).
struct BetLedgerEntry {
    participant_id: Uuid,
    participant_name: String,
    stake: u32,
    payout: u32,
    is_winner: bool,
}

struct BetTally {
    outcomes: Vec<BetOutcomeTotal>,
    entries: Vec<BetLedgerEntry>,
}

/// Groups every response to a bet session by outcome, computing each
/// outcome's live pari-mutuel odds (`pot / stake_on_that_outcome`) and, if
/// `result` is known, each participant's payout. Shared by the public
/// results endpoint, the screen leaderboard, and a participant's stake cap -
/// all three need the same odds/payout math.
async fn tally_bet(
    db: &sqlx::PgPool,
    session_id: Uuid,
    market: &BetMarket,
    result: &Option<BetOutcome>,
) -> Result<BetTally, sqlx::Error> {
    let rows: Vec<BetResponseRow> = sqlx::query_as(
        "SELECT participant_id, participant_name, payload
         FROM interaction_responses
         WHERE session_id = $1
         ORDER BY created_at",
    )
    .bind(session_id)
    .fetch_all(db)
    .await?;

    struct Pick {
        participant_id: Uuid,
        participant_name: String,
        key: OutcomeKey,
        stake: u32,
    }

    let mut stake_by_key: HashMap<OutcomeKey, u32> = HashMap::new();
    let mut order: Vec<OutcomeKey> = Vec::new();
    let mut picks: Vec<Pick> = Vec::new();
    let mut pot: u64 = 0;

    for row in rows {
        let InteractionResponse::Bet { pick, stake } = row.payload.0 else {
            continue;
        };
        let key = OutcomeKey::from_outcome(&pick);
        if !stake_by_key.contains_key(&key) {
            order.push(key.clone());
        }
        *stake_by_key.entry(key.clone()).or_insert(0) += stake;
        pot += u64::from(stake);
        picks.push(Pick {
            participant_id: row.participant_id,
            participant_name: row.participant_name,
            key,
            stake,
        });
    }

    let odds_for = |key: &OutcomeKey| -> Option<f32> {
        let stake = *stake_by_key.get(key)?;
        if stake == 0 {
            None
        } else {
            Some(pot as f32 / stake as f32)
        }
    };

    let outcomes = order
        .iter()
        .map(|key| BetOutcomeTotal {
            label: key.label(market),
            stake_total: stake_by_key.get(key).copied().unwrap_or(0),
            odds: odds_for(key),
        })
        .collect();

    let winning_key = result.as_ref().map(OutcomeKey::from_outcome);
    let winning_odds = winning_key.as_ref().and_then(odds_for);
    let entries = picks
        .into_iter()
        .map(|pick| {
            let is_winner = winning_key.as_ref() == Some(&pick.key);
            let payout = if is_winner {
                winning_odds
                    .map(|o| (pick.stake as f32 * o).round() as u32)
                    .unwrap_or(0)
            } else {
                0
            };
            BetLedgerEntry {
                participant_id: pick.participant_id,
                participant_name: pick.participant_name,
                stake: pick.stake,
                payout,
                is_winner,
            }
        })
        .collect();

    Ok(BetTally { outcomes, entries })
}

async fn fetch_bet_results(
    db: &sqlx::PgPool,
    session_id: Uuid,
    market: &BetMarket,
    result: &Option<BetOutcome>,
) -> Result<InteractionResults, sqlx::Error> {
    let tally = tally_bet(db, session_id, market, result).await?;
    let mut payouts: Vec<BetPayout> = tally
        .entries
        .into_iter()
        .filter(|entry| entry.is_winner)
        .map(|entry| BetPayout {
            participant_id: entry.participant_id.to_string(),
            participant_name: entry.participant_name,
            stake: entry.stake,
            payout: entry.payout,
        })
        .collect();
    payouts.sort_by_key(|p| std::cmp::Reverse(p.payout));

    Ok(InteractionResults::Bet {
        outcomes: tally.outcomes,
        payouts,
    })
}

async fn fetch_results(
    db: &sqlx::PgPool,
    session_id: Uuid,
    interaction: &InteractionKind,
) -> Result<InteractionResults, sqlx::Error> {
    match interaction {
        InteractionKind::Poll { options, .. } => {
            let rows: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT (payload->>'option_index')::bigint AS idx, COUNT(*) AS n
                 FROM interaction_responses
                 WHERE session_id = $1
                 GROUP BY idx",
            )
            .bind(session_id)
            .fetch_all(db)
            .await?;

            let mut counts = vec![0u32; options.len()];
            for (idx, n) in rows {
                if let Some(slot) = usize::try_from(idx).ok().and_then(|i| counts.get_mut(i)) {
                    *slot = n as u32;
                }
            }
            Ok(InteractionResults::Poll { counts })
        }
        InteractionKind::Bet { market, result, .. } => {
            fetch_bet_results(db, session_id, market, result).await
        }
        InteractionKind::Drawing { .. } => {
            let rows: Vec<(SqlJson<InteractionResponse>,)> = sqlx::query_as(
                "SELECT payload FROM interaction_responses
                 WHERE session_id = $1
                 ORDER BY created_at",
            )
            .bind(session_id)
            .fetch_all(db)
            .await?;

            let strokes: Vec<DrawingStroke> = rows
                .into_iter()
                .filter_map(|(payload,)| match payload.0 {
                    InteractionResponse::Drawing { stroke } => Some(stroke),
                    _ => None,
                })
                .collect();
            Ok(InteractionResults::Drawing { strokes })
        }
    }
}

/// Fresh `InteractionResults` for a session, or `None` if the session no
/// longer exists. Shared by the public `GET` handler and the pubsub listener
/// (`crate::pubsub::run_interaction_updates`), which re-fetches results
/// instead of having them embedded in the `NOTIFY` payload.
pub async fn compute_results(
    db: &sqlx::PgPool,
    session_id: Uuid,
) -> Result<Option<InteractionResults>, sqlx::Error> {
    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(db)
            .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(fetch_results(db, session_id, &row.config.0).await?))
}

/// Cumulative leaderboard of every resolved `Bet` slide that has ever run on
/// a screen, summed by participant.
pub async fn compute_screen_leaderboard(
    db: &sqlx::PgPool,
    screen_id: Uuid,
) -> Result<Vec<LeaderboardEntry>, sqlx::Error> {
    let sessions: Vec<(Uuid, SqlJson<InteractionKind>)> = sqlx::query_as(
        "SELECT id, config FROM interaction_sessions WHERE screen_id = $1 AND kind = 'bet'",
    )
    .bind(screen_id)
    .fetch_all(db)
    .await?;

    let resolved_ids: Vec<Uuid> = sessions
        .iter()
        .filter(|(_, config)| {
            matches!(
                &config.0,
                InteractionKind::Bet {
                    result: Some(_),
                    ..
                }
            )
        })
        .map(|(id, _)| *id)
        .collect();

    #[derive(Default)]
    struct Entry {
        participant_name: String,
        net_score: i64,
        bets_played: u32,
        bets_won: u32,
    }
    let mut by_participant: HashMap<Uuid, Entry> = HashMap::new();

    let played_rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT participant_id, participant_name, COUNT(*)
         FROM interaction_responses
         WHERE session_id = ANY($1)
         GROUP BY participant_id, participant_name",
    )
    .bind(&resolved_ids)
    .fetch_all(db)
    .await?;
    for (participant_id, participant_name, played) in played_rows {
        let entry = by_participant.entry(participant_id).or_default();
        entry.participant_name = participant_name;
        entry.bets_played += played as u32;
    }

    for (session_id, config) in &sessions {
        let InteractionKind::Bet {
            market,
            result: result @ Some(_),
            ..
        } = &config.0
        else {
            continue;
        };
        let tally = tally_bet(db, *session_id, market, result).await?;
        for entry in tally.entries {
            let leader = by_participant.entry(entry.participant_id).or_default();
            leader.participant_name = entry.participant_name;
            leader.net_score += i64::from(entry.payout) - i64::from(entry.stake);
            if entry.is_winner {
                leader.bets_won += 1;
            }
        }
    }

    // Fold in resolved gamble rounds (blackjack, etc) the same way - one
    // "bet_played" per round, one "bet_won" per round that returned more
    // than the stake (a push doesn't count as a win), and the net
    // stake/payout difference folded into the same running score as bets.
    let gamble_played_rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT gs.participant_id, gs.participant_name, COUNT(*)
         FROM gamble_seats gs
         JOIN gamble_tables gt ON gt.id = gs.table_id
         WHERE gt.screen_id = $1 AND gs.payout IS NOT NULL
         GROUP BY gs.participant_id, gs.participant_name",
    )
    .bind(screen_id)
    .fetch_all(db)
    .await?;
    for (participant_id, participant_name, played) in gamble_played_rows {
        let entry = by_participant.entry(participant_id).or_default();
        entry.participant_name = participant_name;
        entry.bets_played += played as u32;
    }

    let gamble_payout_rows: Vec<(Uuid, String, i32, i32)> = sqlx::query_as(
        "SELECT gs.participant_id, gs.participant_name, gs.stake, gs.payout
         FROM gamble_seats gs
         JOIN gamble_tables gt ON gt.id = gs.table_id
         WHERE gt.screen_id = $1 AND gs.payout IS NOT NULL",
    )
    .bind(screen_id)
    .fetch_all(db)
    .await?;
    for (participant_id, participant_name, stake, payout) in gamble_payout_rows {
        let entry = by_participant.entry(participant_id).or_default();
        entry.participant_name = participant_name;
        entry.net_score += i64::from(payout) - i64::from(stake);
        if payout > stake {
            entry.bets_won += 1;
        }
    }

    let mut leaderboard: Vec<LeaderboardEntry> = by_participant
        .into_iter()
        .map(|(participant_id, entry)| LeaderboardEntry {
            participant_id: participant_id.to_string(),
            participant_name: entry.participant_name,
            net_score: entry.net_score,
            bets_played: entry.bets_played,
            bets_won: entry.bets_won,
        })
        .collect();
    leaderboard.sort_by_key(|e| std::cmp::Reverse(e.net_score));
    Ok(leaderboard)
}

/// A participant's current net balance on a screen: payouts won minus stakes
/// spent, summed across every resolved bet AND resolved gamble round
/// (blackjack, etc) they've taken part in there. Used to cap how much they
/// may stake next (see `stake_cap`) - not a real wallet (never persisted or
/// topped up), just derived on the fly from history.
pub(crate) async fn compute_participant_balance(
    db: &sqlx::PgPool,
    screen_id: Uuid,
    participant_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let sessions: Vec<(Uuid, SqlJson<InteractionKind>)> = sqlx::query_as(
        "SELECT id, config FROM interaction_sessions WHERE screen_id = $1 AND kind = 'bet'",
    )
    .bind(screen_id)
    .fetch_all(db)
    .await?;

    let mut balance: i64 = 0;
    for (session_id, config) in &sessions {
        let InteractionKind::Bet {
            market,
            result: result @ Some(_),
            ..
        } = &config.0
        else {
            continue;
        };
        let tally = tally_bet(db, *session_id, market, result).await?;
        for entry in tally.entries {
            if entry.participant_id == participant_id {
                balance += i64::from(entry.payout) - i64::from(entry.stake);
            }
        }
    }

    let gamble_rows: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT gs.stake, gs.payout
         FROM gamble_seats gs
         JOIN gamble_tables gt ON gt.id = gs.table_id
         WHERE gt.screen_id = $1 AND gs.participant_id = $2 AND gs.payout IS NOT NULL",
    )
    .bind(screen_id)
    .bind(participant_id)
    .fetch_all(db)
    .await?;
    for (stake, payout) in gamble_rows {
        balance += i64::from(payout) - i64::from(stake);
    }

    Ok(balance)
}

#[derive(serde::Deserialize)]
pub struct GetInteractionQuery {
    #[serde(default)]
    participant_id: Option<String>,
}

pub async fn get_interaction(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<GetInteractionQuery>,
) -> Result<Json<InteractionInfo>, StatusCode> {
    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_uuid)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(row) = row else {
        return Err(StatusCode::NOT_FOUND);
    };

    let interaction = row.config.0;
    let results = fetch_results(&state.db, session_uuid, &interaction)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let participant_id = query
        .participant_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let balance = match participant_id {
        Some(participant_id) => Some(
            compute_participant_balance(&state.db, row.screen_id, participant_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        ),
        None => None,
    };
    let max_stake = match (&interaction, balance) {
        (InteractionKind::Bet { result: None, .. }, Some(balance)) => Some(stake_cap(balance)),
        _ => None,
    };

    Ok(Json(InteractionInfo {
        interaction,
        results,
        max_stake,
        balance,
    }))
}

pub async fn get_screen_leaderboard(
    State(state): State<AppState>,
    Path(screen_id): Path<String>,
) -> Result<Json<Vec<LeaderboardEntry>>, StatusCode> {
    let screen_uuid = Uuid::parse_str(&screen_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let leaderboard = compute_screen_leaderboard(&state.db, screen_uuid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(leaderboard))
}

pub async fn respond(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<InteractionSubmission>,
) -> Result<Json<InteractionResults>, (StatusCode, &'static str)> {
    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID"))?;

    if req.participant_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing participant name"));
    }
    let participant_id = Uuid::parse_str(&req.participant_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid participant ID"))?;

    let row: Option<SessionRow> =
        sqlx::query_as("SELECT screen_id, config FROM interaction_sessions WHERE id = $1")
            .bind(session_uuid)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Session not found"));
    };

    match (&row.config.0, &req.response) {
        (InteractionKind::Poll { options, .. }, InteractionResponse::Poll { option_index }) => {
            if *option_index >= options.len() {
                return Err((StatusCode::BAD_REQUEST, "Invalid option"));
            }
        }
        (InteractionKind::Bet { market, result, .. }, InteractionResponse::Bet { pick, stake }) => {
            if result.is_some() {
                return Err((StatusCode::BAD_REQUEST, "This bet is closed"));
            }
            let balance = compute_participant_balance(&state.db, row.screen_id, participant_id)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
            if *stake < 1 || *stake > stake_cap(balance) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Stake exceeds what you can currently bet",
                ));
            }
            match (market, pick) {
                (BetMarket::Options { options }, BetOutcome::Options { option_index }) => {
                    if *option_index >= options.len() {
                        return Err((StatusCode::BAD_REQUEST, "Invalid option"));
                    }
                }
                (
                    BetMarket::Score { .. },
                    BetOutcome::Score {
                        home_score,
                        away_score,
                    },
                ) => {
                    if *home_score > MAX_SCORE || *away_score > MAX_SCORE {
                        return Err((StatusCode::BAD_REQUEST, "Invalid score"));
                    }
                }
                _ => {
                    return Err((StatusCode::BAD_REQUEST, "Pick doesn't match bet market"));
                }
            }
        }
        (InteractionKind::Drawing { .. }, InteractionResponse::Drawing { stroke }) => {
            if stroke.points.is_empty() || stroke.points.len() > MAX_STROKE_POINTS {
                return Err((StatusCode::BAD_REQUEST, "Invalid stroke"));
            }
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Response type doesn't match interaction kind",
            ))
        }
    }

    let payload = serde_json::to_value(&req.response).unwrap_or_default();
    sqlx::query(
        "INSERT INTO interaction_responses (session_id, participant_id, participant_name, payload)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(session_uuid)
    .bind(participant_id)
    .bind(&req.participant_name)
    .bind(payload)
    .execute(&state.db)
    .await
    .map_err(|err| {
        if matches!(&err, sqlx::Error::Database(db_err) if db_err.is_unique_violation()) {
            (StatusCode::BAD_REQUEST, "You've already placed a bet")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    })?;

    let results = fetch_results(&state.db, session_uuid, &row.config.0)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let devices_to_notify: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM devices WHERE current_screen_id = $1 AND online = TRUE")
            .bind(row.screen_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    for device_id in devices_to_notify {
        let _ = pubsub::notify_interaction_update(&state.db, device_id, session_uuid).await;
    }

    Ok(Json(results))
}
