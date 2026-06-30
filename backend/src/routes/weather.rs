use axum::{extract::Query, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct WeatherQuery {
    pub location: String,
    #[serde(default = "default_days")]
    pub days: u8,
}

fn default_days() -> u8 {
    1
}

#[derive(Serialize)]
pub struct WeatherResponse {
    pub location_name: String,
    pub current_temp: f64,
    pub current_feels_like: f64,
    pub current_weather_code: u16,
    pub wind_speed: f64,
    pub daily: Vec<DailyForecast>,
}

#[derive(Serialize)]
pub struct DailyForecast {
    pub date: String,
    pub temp_max: f64,
    pub temp_min: f64,
    pub weather_code: u16,
}

pub async fn get_weather(
    Query(q): Query<WeatherQuery>,
) -> Result<Json<WeatherResponse>, StatusCode> {
    let days = q.days.clamp(1, 7);

    let client = reqwest::Client::builder()
        .user_agent("DOBoard/1.0")
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let geo: serde_json::Value = client
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[
            ("name", q.location.as_str()),
            ("count", "1"),
            ("language", "fr"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .json()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let result = geo["results"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or(StatusCode::NOT_FOUND)?;

    let lat = result["latitude"]
        .as_f64()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let lon = result["longitude"]
        .as_f64()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let location_name = result["name"]
        .as_str()
        .unwrap_or(q.location.as_str())
        .to_string();

    let days_str = days.to_string();
    let lat_str = lat.to_string();
    let lon_str = lon.to_string();

    let weather: serde_json::Value = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", lat_str.as_str()),
            ("longitude", lon_str.as_str()),
            (
                "current",
                "temperature_2m,apparent_temperature,weather_code,wind_speed_10m",
            ),
            (
                "daily",
                "temperature_2m_max,temperature_2m_min,weather_code",
            ),
            ("forecast_days", days_str.as_str()),
            ("timezone", "auto"),
        ])
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .json()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let current = &weather["current"];
    let daily = &weather["daily"];

    let current_temp = current["temperature_2m"].as_f64().unwrap_or(0.0);
    let current_feels_like = current["apparent_temperature"].as_f64().unwrap_or(0.0);
    let current_weather_code = current["weather_code"].as_u64().unwrap_or(0) as u16;
    let wind_speed = current["wind_speed_10m"].as_f64().unwrap_or(0.0);

    let dates = daily["time"].as_array().cloned().unwrap_or_default();
    let max_temps = daily["temperature_2m_max"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let min_temps = daily["temperature_2m_min"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let codes = daily["weather_code"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let daily_forecasts = dates
        .iter()
        .enumerate()
        .map(|(i, date)| DailyForecast {
            date: date.as_str().unwrap_or("").to_string(),
            temp_max: max_temps.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0),
            temp_min: min_temps.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0),
            weather_code: codes.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        })
        .collect();

    Ok(Json(WeatherResponse {
        location_name,
        current_temp,
        current_feels_like,
        current_weather_code,
        wind_speed,
        daily: daily_forecasts,
    }))
}
