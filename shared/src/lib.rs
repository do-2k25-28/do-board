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
