use crate::{
    gtfs,
    state::{AppState, GtfsCache},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct DeparturesQuery {
    // Accepts one or multiple stop IDs separated by commas: "1080,1115"
    pub stop_id: String,
}

#[derive(Deserialize)]
pub struct StopsQuery {
    pub q: String,
}

#[derive(Serialize, Clone)]
pub struct DepartureInfo {
    pub line_code: String,
    pub line_color: String, // 6-char hex, no leading #
    pub text_color: String,
    pub direction: String,
    pub mode: String,
    pub wait_minutes: i64,
    pub time: String, // "HH:MM" local time
    pub realtime: bool,
}

#[derive(Serialize)]
pub struct DeparturesResponse {
    pub departures: Vec<DepartureInfo>,
}

#[derive(Serialize)]
pub struct StopLineInfo {
    pub code: String,
    pub color: String,
    pub text_color: String,
    pub mode: String,
}

#[derive(Serialize)]
pub struct StopInfo {
    pub id: String,
    pub name: String,
    pub lines: Vec<StopLineInfo>,
}

pub async fn get_departures(
    State(state): State<AppState>,
    Query(q): Query<DeparturesQuery>,
) -> Result<Json<DeparturesResponse>, StatusCode> {
    let stop_ids: Vec<String> = q
        .stop_id
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // 1. Fetch GTFS-RT for all stops (no lock held during network requests).
    let mut raw = Vec::new();
    for sid in &stop_ids {
        raw.extend(gtfs::fetch_rt_for_stop(sid).await);
    }

    // 2. Lock briefly to join with static GTFS data.
    let gtfs_lock = state.gtfs.read().await;
    let gtfs = gtfs_lock.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let now = chrono::Utc::now().timestamp();

    let mut departures: Vec<DepartureInfo> = raw
        .into_iter()
        .filter_map(|rd| {
            let route_id = rd
                .route_id
                .as_deref()
                .or_else(|| gtfs.trips.get(&rd.trip_id).map(|t| t.route_id.as_str()))?;

            let trip_info = gtfs.trips.get(&rd.trip_id);
            let route_info = gtfs.routes.get(route_id);

            let wait_minutes = (rd.dep_time - now) / 60;
            let dt = chrono::DateTime::from_timestamp(rd.dep_time, 0)?;
            let local_dt = dt.with_timezone(&chrono::Local);

            Some(DepartureInfo {
                line_code: route_info
                    .map(|r| r.short_name.clone())
                    .unwrap_or_else(|| "?".into()),
                line_color: route_info
                    .map(|r| r.color.clone())
                    .unwrap_or_else(|| "888888".into()),
                text_color: route_info
                    .map(|r| r.text_color.clone())
                    .unwrap_or_else(|| "FFFFFF".into()),
                direction: trip_info
                    .map(|t| t.headsign.clone())
                    .unwrap_or_else(|| "?".into()),
                mode: route_info
                    .map(|r| gtfs::mode_label(r.route_type).to_string())
                    .unwrap_or_else(|| "Bus".into()),
                wait_minutes,
                time: local_dt.format("%H:%M").to_string(),
                realtime: true,
            })
        })
        .collect();

    departures.sort_by_key(|d| d.wait_minutes);

    if departures.is_empty() {
        let mut all_static: Vec<(i64, DepartureInfo)> = stop_ids
            .iter()
            .flat_map(|sid| collect_static(gtfs, sid, now))
            .collect();
        all_static.sort_by_key(|(ts, _)| *ts);
        all_static.dedup_by(|(_, a), (_, b)| a.line_code == b.line_code && a.time == b.time);
        all_static.truncate(10);
        departures = all_static.into_iter().map(|(_, d)| d).collect();
        eprintln!(
            "[GTFS-Static] Fallback for '{}': {} departures (active_services={})",
            q.stop_id,
            departures.len(),
            gtfs.active_services.len(),
        );
    }

    Ok(Json(DeparturesResponse { departures }))
}

fn collect_static(gtfs: &GtfsCache, stop_id: &str, now: i64) -> Vec<(i64, DepartureInfo)> {
    use chrono::TimeZone;
    let today = chrono::Local::now().date_naive();
    let midnight_ts = chrono::Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| now - now % 86400);

    let Some(times) = gtfs.stop_times.get(stop_id) else {
        return vec![];
    };

    times
        .iter()
        .filter_map(|st| {
            let trip = gtfs.trips.get(&st.trip_id)?;
            if !gtfs.active_services.contains(&trip.service_id) {
                return None;
            }
            let abs_ts = midnight_ts + st.dep_secs as i64;
            if abs_ts < now - 60 || abs_ts > now + 10800 {
                return None;
            }
            let route = gtfs.routes.get(&trip.route_id);
            let dt = chrono::DateTime::from_timestamp(abs_ts, 0)?;
            let local_dt = dt.with_timezone(&chrono::Local);
            Some((
                abs_ts,
                DepartureInfo {
                    line_code: route
                        .map(|r| r.short_name.clone())
                        .unwrap_or_else(|| "?".into()),
                    line_color: route
                        .map(|r| r.color.clone())
                        .unwrap_or_else(|| "888888".into()),
                    text_color: route
                        .map(|r| r.text_color.clone())
                        .unwrap_or_else(|| "FFFFFF".into()),
                    direction: trip.headsign.clone(),
                    mode: route
                        .map(|r| gtfs::mode_label(r.route_type).to_string())
                        .unwrap_or_else(|| "Bus".into()),
                    wait_minutes: (abs_ts - now) / 60,
                    time: local_dt.format("%H:%M").to_string(),
                    realtime: false,
                },
            ))
        })
        .collect()
}

fn stop_lines(gtfs: &GtfsCache, stop_id: &str) -> Vec<StopLineInfo> {
    let mut seen: std::collections::HashMap<String, StopLineInfo> = Default::default();
    if let Some(times) = gtfs.stop_times.get(stop_id) {
        for st in times.iter().take(200) {
            if let Some(trip) = gtfs.trips.get(&st.trip_id) {
                if let Some(route) = gtfs.routes.get(&trip.route_id) {
                    seen.entry(route.short_name.clone())
                        .or_insert_with(|| StopLineInfo {
                            code: route.short_name.clone(),
                            color: route.color.clone(),
                            text_color: route.text_color.clone(),
                            mode: gtfs::mode_label(route.route_type).to_string(),
                        });
                }
            }
        }
    }
    let mut v: Vec<StopLineInfo> = seen.into_values().collect();
    v.sort_by(|a, b| a.code.cmp(&b.code));
    v
}

pub async fn search_stops(
    State(state): State<AppState>,
    Query(q): Query<StopsQuery>,
) -> Result<Json<Vec<StopInfo>>, StatusCode> {
    let gtfs_lock = state.gtfs.read().await;
    let gtfs = gtfs_lock.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let q_lower = q.q.to_lowercase();
    let mut results: Vec<StopInfo> = gtfs
        .stops
        .iter()
        .filter(|(id, name)| {
            name.to_lowercase().contains(&q_lower) && gtfs.stop_times.contains_key(*id)
        })
        .map(|(id, name)| {
            let lines = stop_lines(gtfs, id);
            StopInfo {
                id: id.clone(),
                name: name.clone(),
                lines,
            }
        })
        .collect();

    results.sort_by(|a, b| a.name.cmp(&b.name));
    results.truncate(10);
    Ok(Json(results))
}
