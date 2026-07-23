use std::{
    cell::OnceCell,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ffxivfishing::{
    carbuncledata,
    eorzea_time::EorzeaTime,
    fish::{Fish, FishData},
    weather::Weather,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

thread_local! {
    static FISH_DATA: OnceCell<FishData> = const { OnceCell::new() };
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FishInfo {
    id: u32,
    name: String,
    location: String,
    region: String,
    tug: String,
    hookset: String,
    bait_id: Option<u32>,
    window_start: String,
    window_end: String,
    weather_set: Vec<String>,
    patch: (u8, u8),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FishWindow {
    start_esec: u64,
    end_esec: u64,
    start_display: String,
    end_display: String,
    duration_esec: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FishEntry {
    id: u32,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeatherInfo {
    weather: String,
    timestamp_esec: u64,
    timestamp_display: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EorzeaTimeInfo {
    esec: u64,
    display: String,
    year: u16,
    moon: u8,
    sun: u8,
    bell: u8,
    minute: u8,
    second: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleEntry {
    #[serde(default)]
    day_of_week: Option<u8>,
    start_sec: u64,
    end_sec: u64,
}

fn weather_to_string(w: &Weather) -> String {
    match w {
        Weather::Unknown => "Unknown".to_string(),
        Weather::Id(id) => format!("Id({})", id),
        Weather::Sunny => "Sunny".to_string(),
        Weather::Clouds => "Clouds".to_string(),
        Weather::ClearSkies => "ClearSkies".to_string(),
        Weather::FairSkies => "FairSkies".to_string(),
        Weather::Fog => "Fog".to_string(),
        Weather::Wind => "Wind".to_string(),
    }
}

fn with_fish_data<T>(f: impl FnOnce(&FishData) -> Result<T, JsValue>) -> Result<T, JsValue> {
    FISH_DATA.with(|cell| {
        cell.get()
            .ok_or_else(|| JsValue::from_str("Not initialized. Call init() first."))
            .and_then(f)
    })
}

fn fish_to_info(fish: &Fish) -> FishInfo {
    FishInfo {
        id: fish.id,
        name: fish.name.clone(),
        location: fish.location.name().to_string(),
        region: fish.location.region().name().to_string(),
        tug: fish.tug.to_string(),
        hookset: fish.hookset.to_string(),
        bait_id: fish.bait_id(),
        window_start: fish.window_start.to_string(),
        window_end: fish.window_end.to_string(),
        weather_set: fish.weather_set.iter().map(weather_to_string).collect(),
        patch: fish.patch,
    }
}

fn window_overlaps_any_schedule(
    win_start_rt: SystemTime,
    win_end_rt: SystemTime,
    schedule: &[ScheduleEntry],
) -> bool {
    let start_secs = win_start_rt
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let end_secs = win_end_rt
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let start_day = start_secs / 86400;
    let end_day = end_secs / 86400;

    for day in start_day..=end_day {
        let day_midnight = day * 86400;
        let day_of_week = ((day + 4) % 7) as u8;

        for entry in schedule {
            if entry
                .day_of_week
                .map_or_else(|| true, |dow| dow == day_of_week)
            {
                let sched_start = day_midnight + entry.start_sec;
                let sched_end = day_midnight + entry.end_sec;

                if start_secs < sched_end && end_secs > sched_start {
                    return true;
                }
            }
        }
    }
    false
}

#[wasm_bindgen]
pub fn init(data_json: &str) -> Result<(), JsValue> {
    let fish_data = carbuncledata::carbuncle_fishes_from_str(data_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    FISH_DATA.with(|cell| {
        cell.set(fish_data)
            .map_err(|_| JsValue::from_str("Already initialized"))
    })
}

#[wasm_bindgen]
pub fn init_default() -> Result<(), JsValue> {
    let fish_data =
        carbuncledata::carbuncle_fishes().map_err(|e| JsValue::from_str(&e.to_string()))?;
    FISH_DATA.with(|cell| {
        cell.set(fish_data)
            .map_err(|_| JsValue::from_str("Already initialized"))
    })
}

#[wasm_bindgen]
pub fn get_fish(fish_id: u32) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        serde_json::to_string(&fish_to_info(fish)).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_fish_windows(fish_id: u32, timestamp_esec: u64, limit: u32) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        let eorzea_time = EorzeaTime::from_esecs(timestamp_esec);
        let mut windows: Vec<FishWindow> = Vec::new();
        let mut current_time = eorzea_time;
        let mut remaining = limit;
        while remaining > 0 {
            if let Some(window) = fish.next_window(current_time, false, remaining) {
                windows.push(FishWindow {
                    start_esec: window.start().as_esecs(),
                    end_esec: window.end().as_esecs(),
                    start_display: window.start().to_string(),
                    end_display: window.end().to_string(),
                    duration_esec: window.duration().total_seconds(),
                });
                current_time = window.end();
                remaining -= 1;
            } else {
                break;
            }
        }
        serde_json::to_string(&windows).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_fish_windows_in_schedule(
    fish_id: u32,
    timestamp_esec: u64,
    schedule_json: &str,
    timeperiod_secs: u64,
) -> Result<String, JsValue> {
    let schedule: Vec<ScheduleEntry> = serde_json::from_str(schedule_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid schedule JSON: {}", e)))?;

    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        let now_et = EorzeaTime::from_esecs(timestamp_esec);
        let now_rt = now_et.to_system_time();
        let end_rt = now_rt + Duration::from_secs(timeperiod_secs);

        let mut windows: Vec<FishWindow> = Vec::new();
        let mut current_et = now_et;
        let max_lookahead = 10000u32;

        while let Some(fw) = fish.next_window(current_et, false, max_lookahead) {
            let fw_start_rt = fw.start().to_system_time();
            let fw_end_rt = fw.end().to_system_time();

            if fw_start_rt > end_rt {
                break;
            }

            if fw_end_rt > now_rt && window_overlaps_any_schedule(fw_start_rt, fw_end_rt, &schedule)
            {
                windows.push(FishWindow {
                    start_esec: fw.start().as_esecs(),
                    end_esec: fw.end().as_esecs(),
                    start_display: fw.start().to_string(),
                    end_display: fw.end().to_string(),
                    duration_esec: fw.duration().total_seconds(),
                });
            }

            current_et = fw.end();
        }

        serde_json::to_string(&windows).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_weather_at_fish(fish_id: u32, timestamp_esec: u64) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        let eorzea_time = EorzeaTime::from_esecs(timestamp_esec);
        let weather = fish.location.region().weather().weather_at(eorzea_time);
        let info = WeatherInfo {
            weather: weather_to_string(weather),
            timestamp_esec,
            timestamp_display: eorzea_time.to_string(),
        };
        serde_json::to_string(&info).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn search_fish(query: &str) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let results: Vec<FishEntry> = fd
            .search_fish(query)
            .into_iter()
            .map(|(id, name)| FishEntry { id, name })
            .collect();
        serde_json::to_string(&results).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn list_all_fish() -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let results: Vec<FishEntry> = fd
            .fishes()
            .iter()
            .map(|f| FishEntry {
                id: f.id,
                name: f.name.clone(),
            })
            .collect();
        serde_json::to_string(&results).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn eorzea_time_from_unix(unix_secs: u64) -> String {
    let et = EorzeaTime::from_esecs(unix_secs);
    let info = EorzeaTimeInfo {
        esec: et.as_esecs(),
        display: et.to_string(),
        year: et.year(),
        moon: et.moon(),
        sun: et.sun(),
        bell: et.bell(),
        minute: et.minute(),
        second: et.second(),
    };
    serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string())
}

#[wasm_bindgen]
pub fn unix_from_eorzea_time(esec: u64) -> u64 {
    let et = EorzeaTime::from_esecs(esec);
    et.to_system_time()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[wasm_bindgen]
pub fn unix_to_eorzea_esec(unix_secs: u64) -> u64 {
    ((unix_secs as f64) * 3600.0 / 175.0).round() as u64
}

