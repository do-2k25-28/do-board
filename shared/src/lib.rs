use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub ip: String,
    pub browser: String,
    pub os: String,
    pub online: bool,
    pub connected_at: String,
    pub last_seen: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub saved: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveDeviceRequest {
    pub name: String,
}

// ── Screens & Slides ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    pub name: String,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub theme: ScreenTheme,
}

/// Preset font stacks for on-screen text. Kept to system/web-safe fonts so
/// signage never depends on a remote font fetch succeeding.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenFont {
    #[default]
    Sans,
    Serif,
    Mono,
    Display,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScreenTheme {
    /// CSS color (e.g. `#0a0a0a`). `None` = default dark background.
    #[serde(default)]
    pub background_color: Option<String>,
    /// Path to an uploaded image, e.g. `/api/media/{id}`. Takes priority over
    /// `background_color` when set.
    #[serde(default)]
    pub background_image_url: Option<String>,
    /// CSS color (e.g. `#ffffff`). `None` = default white text.
    #[serde(default)]
    pub text_color: Option<String>,
    #[serde(default)]
    pub font: ScreenFont,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushScreenRequest {
    pub screen_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SlideTransition {
    None,
    #[default]
    Fade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Zoom,
}

fn default_weather_days() -> u8 {
    1
}

fn default_transition_duration_ms() -> u32 {
    500
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    pub duration_secs: u32,
    pub config: SlideConfig,
    #[serde(default)]
    pub transition: SlideTransition,
    #[serde(default = "default_transition_duration_ms")]
    pub transition_duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlideConfig {
    Weather {
        location: String,
        #[serde(default = "default_weather_days")]
        days: u8,
    },
    Transport {
        provider: TransportProvider,
        stop_id: String,
        stop_name: String,
        #[serde(default)]
        extra_stop_ids: Vec<String>,
    },
    Birthdays {
        entries: Vec<BirthdayEntry>,
    },
    Iframe {
        url: String,
        #[serde(default)]
        cookies: Vec<KvEntry>,
        #[serde(default)]
        local_storage: Vec<KvEntry>,
        /// Vertical scroll position as a percentage (0-100) of the page's
        /// scrollable height. 0 = top, 50 = middle, 100 = bottom.
        #[serde(default)]
        scroll_y_percent: u8,
    },
    Clock {
        clocks: Vec<ClockConfig>,
    },
    Image {
        /// Path to an uploaded image, e.g. `/api/media/{id}`.
        url: String,
    },
    Video {
        /// A YouTube URL (watch/share/embed/shorts link) or bare video ID.
        url: String,
    },
    Interactive {
        /// Stable uuid, generated once when the slide is created. Embedded in
        /// the join URL/QR code shown on screen - must never be regenerated
        /// on save, or existing QR codes/links stop working.
        session_id: String,
        interaction: InteractionKind,
    },
    /// Passive display of a screen's cumulative bet leaderboard - no config
    /// of its own, fed by `GET /api/screens/{screen_id}/leaderboard`.
    Leaderboard {},
}

/// Config for an interactive slide, keyed by `kind` in JSON. Each variant is
/// the type-specific setup an editor fills in (question/options, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionKind {
    Poll {
        question: String,
        options: Vec<String>,
    },
    Bet {
        question: String,
        market: BetMarket,
        /// Set by the admin once the real outcome is known, via the same
        /// screen-save flow as any other slide edit. `None` while betting is
        /// still open; once set, new responses are rejected.
        #[serde(default)]
        result: Option<BetOutcome>,
    },
    Drawing {
        prompt: String,
    },
}

/// What a `Bet` slide's participants are predicting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetMarket {
    Options {
        options: Vec<String>,
    },
    Score {
        home_label: String,
        away_label: String,
    },
}

/// A single predicted (or actual) outcome. Reused both for a participant's
/// pick and for the admin-entered real result, since they share the same
/// shape - `Score { home_score, away_score }` is a scoreline whether it's a
/// guess or the final tally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetOutcome {
    Options { option_index: usize },
    Score { home_score: u32, away_score: u32 },
}

/// One freehand stroke on a shared drawing canvas. `points` are normalized to
/// 0.0-1.0 so a stroke drawn on one device's canvas scales cleanly onto a
/// canvas of any other size (phone vs. screen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingStroke {
    pub color: String,
    pub points: Vec<[f32; 2]>,
}

/// Live aggregate results for an interaction session, pushed to the screen
/// over the WebSocket and returned by the public join-page lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionResults {
    Poll {
        counts: Vec<u32>,
    },
    Bet {
        outcomes: Vec<BetOutcomeTotal>,
        /// Winners and their payout, computed once the admin has entered the
        /// real result. Empty while the bet is still open.
        payouts: Vec<BetPayout>,
    },
    Drawing {
        strokes: Vec<DrawingStroke>,
    },
}

/// Live pari-mutuel tally for one possible outcome of a `Bet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BetOutcomeTotal {
    pub label: String,
    pub stake_total: u32,
    /// `pot / stake_total` - the live payout multiplier for this outcome.
    /// `None` if nobody has staked on it yet (undefined).
    pub odds: Option<f32>,
}

/// One winner's payout for a resolved `Bet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BetPayout {
    pub participant_id: String,
    pub participant_name: String,
    pub stake: u32,
    pub payout: u32,
}

/// One row of a screen's cumulative bet leaderboard, summed across every
/// resolved `Bet` slide that has ever run on that screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub participant_id: String,
    pub participant_name: String,
    pub total_payout: u32,
    pub bets_played: u32,
    pub bets_won: u32,
}

/// Returned by `GET /api/interact/{session_id}` for the public join page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionInfo {
    pub interaction: InteractionKind,
    pub results: InteractionResults,
    /// The most the requesting participant may currently stake on this bet.
    /// Only set for an open `Bet` when the request identified the
    /// participant (`?participant_id=`); `None` otherwise (Poll, Drawing, or
    /// an anonymous/screen-side lookup).
    #[serde(default)]
    pub max_stake: Option<u32>,
}

/// Body of `POST /api/interact/{session_id}/respond`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSubmission {
    pub participant_name: String,
    /// Stable id generated and persisted (e.g. in localStorage) on the
    /// participant's device, so their score can be tracked across multiple
    /// bets on the same screen.
    pub participant_id: String,
    pub response: InteractionResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionResponse {
    Poll { option_index: usize },
    Bet { pick: BetOutcome, stake: u32 },
    Drawing { stroke: DrawingStroke },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportProvider {
    Tam,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BirthdayEntry {
    pub name: String,
    /// dd-mm-yyyy format
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockConfig {
    pub timezone: String,
    #[serde(default)]
    pub label: Option<String>,
    pub style: ClockStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockStyle {
    Digital,
    Analog,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateScreenRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateScreenRequest {
    pub name: String,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub theme: ScreenTheme,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user: User,
    /// Unix timestamp (seconds) the session cookie expires at. Informational
    /// only, for client-side UX (e.g. proactively redirecting to /login) -
    /// the actual JWT lives in an HttpOnly cookie the client never sees.
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Used by an admin to reset another user's password - no current password
/// required, unlike `ChangePasswordRequest` which is for self-service.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetPasswordRequest {
    pub new_password: String,
}
