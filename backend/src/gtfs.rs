/// Static GTFS loading and GTFS-RT protobuf decoding for TaM Montpellier.
///
/// Configure via env vars:
///   GTFS_STATIC_URL - ZIP download URL (refreshed daily)
///   GTFS_RT_URL     - binary GTFS-RT TripUpdate protobuf URL
use crate::state::{GtfsCache, RouteInfo, TripInfo};
use prost::Message;
use serde::Deserialize;
use std::{collections::HashMap, io::Cursor};

// ── GTFS-RT protobuf structs (official GTFS-RT field numbers) ─────────────────

#[derive(Clone, PartialEq, prost::Message)]
pub struct FeedMessage {
    #[prost(message, optional, tag = "1")]
    pub header: Option<FeedHeader>,
    #[prost(message, repeated, tag = "2")]
    pub entity: Vec<FeedEntity>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FeedHeader {
    #[prost(string, tag = "1")]
    pub gtfs_realtime_version: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FeedEntity {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, optional, tag = "3")]
    pub trip_update: Option<TripUpdate>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TripUpdate {
    #[prost(message, optional, tag = "1")]
    pub trip: Option<TripDescriptor>,
    #[prost(message, repeated, tag = "2")]
    pub stop_time_update: Vec<StopTimeUpdate>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TripDescriptor {
    #[prost(string, optional, tag = "1")]
    pub trip_id: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub route_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StopTimeUpdate {
    #[prost(uint32, optional, tag = "1")]
    pub stop_sequence: Option<u32>,
    #[prost(message, optional, tag = "2")]
    pub arrival: Option<StopTimeEvent>,
    /// departure is field 3 per GTFS-RT spec
    #[prost(message, optional, tag = "3")]
    pub departure: Option<StopTimeEvent>,
    #[prost(string, optional, tag = "4")]
    pub stop_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StopTimeEvent {
    #[prost(int32, optional, tag = "1")]
    pub delay: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub time: Option<i64>,
}

// ── Raw departure (pre-join with static GTFS) ─────────────────────────────────

pub struct RtDeparture {
    pub trip_id: String,
    pub route_id: Option<String>,
    pub dep_time: i64,
}

// ── Static GTFS CSV deserialisation ───────────────────────────────────────────

#[derive(Deserialize)]
struct CsvStop {
    stop_id: String,
    stop_name: String,
}

#[derive(Deserialize)]
struct CsvRoute {
    route_id: String,
    route_short_name: String,
    route_color: String,
    route_text_color: String,
    route_type: String,
}

#[derive(Deserialize)]
struct CsvTrip {
    trip_id: String,
    route_id: String,
    service_id: String,
    trip_headsign: Option<String>,
}

#[derive(Deserialize)]
struct CsvStopTime {
    trip_id: String,
    departure_time: String,
    stop_id: String,
}

#[derive(Deserialize)]
struct CsvCalendar {
    service_id: String,
    monday: u8,
    tuesday: u8,
    wednesday: u8,
    thursday: u8,
    friday: u8,
    saturday: u8,
    sunday: u8,
    start_date: String,
    end_date: String,
}

#[derive(Deserialize)]
struct CsvCalendarDate {
    service_id: String,
    date: String,
    exception_type: u8,
}

// ── Public helpers ─────────────────────────────────────────────────────────────

pub fn gtfs_rt_url() -> String {
    std::env::var("GTFS_RT_URL").unwrap_or_else(|_| {
        "https://data.montpellier3m.fr/TAM_MMM_GTFSRT/TripUpdate.pb".to_string()
    })
}

pub fn gtfs_static_url() -> Option<String> {
    std::env::var("GTFS_STATIC_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn mode_label(route_type: u16) -> &'static str {
    match route_type {
        0 => "Tramway",
        1 => "Metro",
        2 => "Rail",
        _ => "Bus",
    }
}

fn parse_gtfs_time(s: &str) -> Option<u32> {
    let s = s.trim();
    let mut parts = s.splitn(3, ':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let sec: u32 = parts.next()?.parse().ok()?;
    Some(h * 3600 + m * 60 + sec)
}

/// Download and parse the static GTFS ZIP from `url`.
pub async fn load_static(url: &str) -> Option<GtfsCache> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .ok()?;

    let bytes = client.get(url).send().await.ok()?.bytes().await.ok()?;
    let cursor = Cursor::new(bytes.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).ok()?;

    let stops = parse_stops(&mut archive);
    let routes = parse_routes(&mut archive);
    let trips = parse_trips(&mut archive);
    let stop_times = parse_stop_times(&mut archive);
    let today = chrono::Local::now().date_naive();
    let active_services = compute_active_services(&mut archive, today);

    eprintln!(
        "[GTFS] Loaded: {} stops, {} routes, {} trips, {} stop_time entries, {} active services today",
        stops.len(),
        routes.len(),
        trips.len(),
        stop_times.len(),
        active_services.len(),
    );

    Some(GtfsCache {
        stops,
        trips,
        routes,
        stop_times,
        active_services,
    })
}

/// Fetch GTFS-RT protobuf and return raw departures for `stop_id`,
/// sorted ascending by departure time.
pub async fn fetch_rt_for_stop(stop_id: &str) -> Vec<RtDeparture> {
    let url = gtfs_rt_url();

    let bytes = match async { reqwest::get(&url).await?.bytes().await }.await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[GTFS-RT] fetch error: {e}");
            return vec![];
        }
    };

    let feed = match FeedMessage::decode(bytes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[GTFS-RT] decode error: {e}");
            return vec![];
        }
    };

    let now = chrono::Utc::now().timestamp();
    let mut out: Vec<RtDeparture> = Vec::new();
    let mut sample_ids: Vec<String> = Vec::new();

    for entity in &feed.entity {
        let Some(tu) = &entity.trip_update else {
            continue;
        };
        let trip_id = tu
            .trip
            .as_ref()
            .and_then(|t| t.trip_id.as_deref())
            .unwrap_or("")
            .to_string();
        let route_id = tu.trip.as_ref().and_then(|t| t.route_id.clone());

        for stu in &tu.stop_time_update {
            // Collect a sample of stop IDs for diagnostics.
            if sample_ids.len() < 10 {
                if let Some(sid) = stu.stop_id.as_deref() {
                    if !sample_ids.contains(&sid.to_string()) {
                        sample_ids.push(sid.to_string());
                    }
                }
            }

            if stu.stop_id.as_deref() != Some(stop_id) {
                continue;
            }
            // Use departure.time, falling back to arrival.time (common in French feeds).
            let dep_time = stu
                .departure
                .as_ref()
                .and_then(|d| d.time)
                .or_else(|| stu.arrival.as_ref().and_then(|a| a.time));
            let Some(dep_time) = dep_time else {
                continue;
            };
            if dep_time < now - 60 {
                continue; // already left
            }
            out.push(RtDeparture {
                trip_id: trip_id.clone(),
                route_id: route_id.clone(),
                dep_time,
            });
        }
    }

    if out.is_empty() {
        eprintln!(
            "[GTFS-RT] No departures for stop '{}'. Feed has {} entities. Sample stop IDs: {:?}",
            stop_id,
            feed.entity.len(),
            sample_ids,
        );
    } else {
        eprintln!(
            "[GTFS-RT] Found {} departure(s) for stop '{}'",
            out.len(),
            stop_id
        );
    }

    out.sort_by_key(|d| d.dep_time);
    out
}

// ── CSV parsers ────────────────────────────────────────────────────────────────

fn read_file(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Option<Vec<u8>> {
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut buf).ok()?;
    Some(buf)
}

fn parse_stops(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>) -> HashMap<String, String> {
    let bytes = match read_file(archive, "stops.txt") {
        Some(b) => b,
        None => return HashMap::new(),
    };
    csv::Reader::from_reader(bytes.as_slice())
        .deserialize::<CsvStop>()
        .filter_map(|r| r.ok())
        .map(|s| (s.stop_id, s.stop_name))
        .collect()
}

fn parse_routes(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>) -> HashMap<String, RouteInfo> {
    let bytes = match read_file(archive, "routes.txt") {
        Some(b) => b,
        None => return HashMap::new(),
    };
    csv::Reader::from_reader(bytes.as_slice())
        .deserialize::<CsvRoute>()
        .filter_map(|r| r.ok())
        .map(|r| {
            let route_type: u16 = r.route_type.parse().unwrap_or(3);
            (
                r.route_id,
                RouteInfo {
                    short_name: r.route_short_name,
                    color: normalize_color(&r.route_color),
                    text_color: normalize_color(&r.route_text_color),
                    route_type,
                },
            )
        })
        .collect()
}

fn parse_trips(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>) -> HashMap<String, TripInfo> {
    let bytes = match read_file(archive, "trips.txt") {
        Some(b) => b,
        None => return HashMap::new(),
    };
    csv::Reader::from_reader(bytes.as_slice())
        .deserialize::<CsvTrip>()
        .filter_map(|r| r.ok())
        .map(|t| {
            (
                t.trip_id,
                TripInfo {
                    route_id: t.route_id,
                    headsign: t.trip_headsign.unwrap_or_default(),
                    service_id: t.service_id,
                },
            )
        })
        .collect()
}

fn parse_stop_times(
    archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>,
) -> HashMap<String, Vec<crate::state::StopTimeDep>> {
    use crate::state::StopTimeDep;
    let bytes = match read_file(archive, "stop_times.txt") {
        Some(b) => b,
        None => return HashMap::new(),
    };
    let mut map: HashMap<String, Vec<StopTimeDep>> = HashMap::new();
    for rec in csv::Reader::from_reader(bytes.as_slice())
        .deserialize::<CsvStopTime>()
        .filter_map(|r| r.ok())
    {
        if let Some(dep_secs) = parse_gtfs_time(&rec.departure_time) {
            map.entry(rec.stop_id).or_default().push(StopTimeDep {
                trip_id: rec.trip_id,
                dep_secs,
            });
        }
    }
    map
}

fn compute_active_services(
    archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>,
    today: chrono::NaiveDate,
) -> std::collections::HashSet<String> {
    use chrono::Datelike;
    let today_str = today.format("%Y%m%d").to_string();
    let weekday = today.weekday();
    let mut services: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(bytes) = read_file(archive, "calendar.txt") {
        for rec in csv::Reader::from_reader(bytes.as_slice())
            .deserialize::<CsvCalendar>()
            .filter_map(|r| r.ok())
        {
            if rec.start_date.as_str() <= today_str.as_str()
                && today_str.as_str() <= rec.end_date.as_str()
            {
                let runs = match weekday {
                    chrono::Weekday::Mon => rec.monday,
                    chrono::Weekday::Tue => rec.tuesday,
                    chrono::Weekday::Wed => rec.wednesday,
                    chrono::Weekday::Thu => rec.thursday,
                    chrono::Weekday::Fri => rec.friday,
                    chrono::Weekday::Sat => rec.saturday,
                    chrono::Weekday::Sun => rec.sunday,
                };
                if runs == 1 {
                    services.insert(rec.service_id);
                }
            }
        }
    }

    if let Some(bytes) = read_file(archive, "calendar_dates.txt") {
        for rec in csv::Reader::from_reader(bytes.as_slice())
            .deserialize::<CsvCalendarDate>()
            .filter_map(|r| r.ok())
        {
            if rec.date == today_str {
                match rec.exception_type {
                    1 => {
                        services.insert(rec.service_id);
                    }
                    2 => {
                        services.remove(&rec.service_id);
                    }
                    _ => {}
                }
            }
        }
    }

    services
}

fn normalize_color(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        h.to_uppercase()
    } else {
        "888888".to_string()
    }
}
