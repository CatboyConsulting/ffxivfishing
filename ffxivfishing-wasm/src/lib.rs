use std::{
    cell::OnceCell,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{Offset, TimeZone, Utc};
use chrono_tz::Tz;
use ffxivfishing::{
    carbuncledata,
    eorzea_time::{EORZEA_SUN, EorzeaTime},
    fish::{DEFAULT_INTUITION_LOOKBACK_MINUTES, Fish, FishData},
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

thread_local! {
    static FISH_DATA: OnceCell<FishData> = const { OnceCell::new() };
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntuitionRequirementInfo {
    amount: u8,
    fish: String,
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
    bait: Option<String>,
    mooch_path: Vec<String>,
    snagging: bool,
    lure: Option<String>,
    lure_proc: bool,
    fish_eyes: bool,
    big_fish: bool,
    window_start: String,
    window_end: String,
    previous_weather_set: Vec<String>,
    weather_set: Vec<String>,
    previous_weather_uptime: f64,
    weather_uptime: f64,
    pattern_uptime: f64,
    fish_uptime: f64,
    patch: String,
    intuition_requirements: Vec<IntuitionRequirementInfo>,
    intuition_length_seconds: Option<u64>,
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

fn with_fish_data<T>(f: impl FnOnce(&FishData) -> Result<T, JsValue>) -> Result<T, JsValue> {
    FISH_DATA.with(|cell| {
        cell.get()
            .ok_or_else(|| JsValue::from_str("Not initialized. Call init() first."))
            .and_then(f)
    })
}

fn fish_to_info(fish: &Fish, fd: &FishData) -> FishInfo {
    let p = fish.patch;
    let patch_str = if p.1.is_multiple_of(10) {
        format!("{}.{}", p.0, p.1 / 10)
    } else {
        format!("{}.{}", p.0, p.1)
    };
    let weather = fish.location.region().weather();
    let prev_uptime = weather.weather_uptime(&fish.previous_weather_set);
    let curr_uptime = weather.weather_uptime(&fish.weather_set);
    let pat_uptime = weather.pattern_uptime(&fish.previous_weather_set, &fish.weather_set);

    let day = EORZEA_SUN.total_seconds();
    let start = fish.window_start.total_seconds();
    let end = fish.window_end.total_seconds();
    let window_len = if end > start {
        end - start
    } else {
        end + day - start
    };
    let fish_uptime = pat_uptime * (window_len as f64 / day as f64);
    let item_name = |id: u32| {
        fd.item_by_id(id)
            .map(|item| item.name().to_string())
            .unwrap_or_else(|| format!("Unknown item ({id})"))
    };
    let intuition_requirements = fish
        .intuition_requirements()
        .unwrap_or_default()
        .iter()
        .map(|(amount, id)| IntuitionRequirementInfo {
            amount: *amount,
            fish: item_name(*id),
        })
        .collect();
    let bait = fish.base_bait_id().map(item_name);
    let mooch_path = fish
        .mooch_path()
        .unwrap_or_default()
        .iter()
        .map(|id| item_name(*id))
        .collect();
    let lure = fish.lure_proc.then(|| fish.lure.to_string());

    FishInfo {
        id: fish.id,
        name: fish.name.clone(),
        location: fish.location.name().to_string(),
        region: fish.location.region().name().to_string(),
        tug: fish.tug.to_string(),
        hookset: fish.hookset.to_string(),
        bait_id: fish.bait_id(),
        bait,
        mooch_path,
        snagging: fish.snagging,
        lure,
        lure_proc: fish.lure_proc,
        fish_eyes: fish.fish_eyes,
        big_fish: fish.big_fish,
        window_start: fish.window_start.to_string(),
        window_end: fish.window_end.to_string(),
        previous_weather_set: fish
            .previous_weather_set
            .iter()
            .map(|w| fd.weather_name(w))
            .collect(),
        weather_set: fish
            .weather_set
            .iter()
            .map(|w| fd.weather_name(w))
            .collect(),
        previous_weather_uptime: prev_uptime,
        weather_uptime: curr_uptime,
        pattern_uptime: pat_uptime,
        fish_uptime: fish_uptime,
        patch: patch_str,
        intuition_requirements,
        intuition_length_seconds: fish.intuition_length_seconds(),
    }
}

fn window_overlaps_any_schedule(
    win_start_rt: SystemTime,
    win_end_rt: SystemTime,
    schedule: &[ScheduleEntry],
    tz: Tz,
) -> bool {
    let start_secs = win_start_rt
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let end_secs = win_end_rt
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let win_start_day = start_secs / 86400;
    let win_end_day = end_secs / 86400;

    for day in win_start_day..=win_end_day {
        let day_midnight = day * 86400;
        let day_end = day_midnight + 86400;
        let day_of_week = ((day + 4) % 7) as u8;

        let portion_start = start_secs.max(day_midnight);
        let portion_end = end_secs.min(day_end);
        if portion_start >= portion_end {
            continue;
        }

        let dt = Utc.timestamp_opt(day_midnight as i64, 0).single().unwrap();
        let local_minus_utc = tz
            .offset_from_utc_datetime(&dt.naive_utc())
            .fix()
            .local_minus_utc();
        let offset = -local_minus_utc as i64;

        let local_start = portion_start as i64 - offset;
        let local_end = portion_end as i64 - offset;
        let local_day_start = (local_start / 86400) * 86400;

        for entry in schedule {
            if entry
                .day_of_week
                .map_or_else(|| true, |dow| dow == day_of_week)
            {
                let sched_start = local_day_start + entry.start_sec as i64;
                let mut sched_end = local_day_start + entry.end_sec as i64;
                if entry.end_sec <= entry.start_sec {
                    sched_end += 86400;
                }

                if local_start < sched_end && local_end > sched_start {
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
        serde_json::to_string(&fish_to_info(fish, fd))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_fish_next_window(
    fish_id: u32,
    timestamp_esec: u64,
    filter_intuition: bool,
    use_fish_eyes: bool,
) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        let eorzea_time = EorzeaTime::from_esecs(timestamp_esec);
        let max_lookahead = 10000u32;
        let window = fish.next_window(
            eorzea_time,
            true,
            filter_intuition,
            use_fish_eyes,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
            max_lookahead,
        );
        match window {
            Some(fw) => serde_json::to_string(&Some(FishWindow {
                start_esec: fw.start().as_esecs(),
                end_esec: fw.end().as_esecs(),
                start_display: fw.start().to_string(),
                end_display: fw.end().to_string(),
                duration_esec: fw.duration().total_seconds(),
            })),
            None => serde_json::to_string::<Option<FishWindow>>(&None),
        }
        .map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_fish_windows(
    fish_id: u32,
    timestamp_esec: u64,
    limit: u32,
    filter_intuition: bool,
    use_fish_eyes: bool,
    include_ongoing: bool,
) -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let fish = fd
            .fish_by_id(fish_id)
            .ok_or_else(|| JsValue::from_str("Fish not found"))?;
        let eorzea_time = EorzeaTime::from_esecs(timestamp_esec);
        let mut windows: Vec<FishWindow> = Vec::new();
        let mut current_time = eorzea_time;
        let mut remaining = limit;
        let mut include_current_ongoing = include_ongoing;
        while remaining > 0 {
            if let Some(window) = fish.next_window(
                current_time,
                include_current_ongoing,
                filter_intuition,
                use_fish_eyes,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                remaining,
            ) {
                windows.push(FishWindow {
                    start_esec: window.start().as_esecs(),
                    end_esec: window.end().as_esecs(),
                    start_display: window.start().to_string(),
                    end_display: window.end().to_string(),
                    duration_esec: window.duration().total_seconds(),
                });
                current_time = window.end();
                include_current_ongoing = false;
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
    timezone_name: &str,
    filter_intuition: bool,
    use_fish_eyes: bool,
    include_ongoing: bool,
) -> Result<String, JsValue> {
    let local_schedule: Vec<ScheduleEntry> = serde_json::from_str(schedule_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid schedule JSON: {}", e)))?;
    let tz: Tz = timezone_name
        .parse()
        .map_err(|_| JsValue::from_str("Invalid timezone name"))?;

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
        let mut include_current_ongoing = include_ongoing;

        while let Some(fw) = fish.next_window(
            current_et,
            include_current_ongoing,
            filter_intuition,
            use_fish_eyes,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
            max_lookahead,
        ) {
            let fw_start_rt = fw.start().to_system_time();
            let fw_end_rt = fw.end().to_system_time();

            if fw_start_rt > end_rt {
                break;
            }

            if fw_end_rt > now_rt
                && window_overlaps_any_schedule(fw_start_rt, fw_end_rt, &local_schedule, tz)
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
            include_current_ongoing = false;
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
            weather: fd.weather_name(weather),
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
pub fn list_all_fish_info() -> Result<String, JsValue> {
    with_fish_data(|fd| {
        let results: Vec<FishInfo> = fd.fishes().iter().map(|f| fish_to_info(f, fd)).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn tz() -> Tz {
        "Europe/London".parse().unwrap()
    }

    fn bst_secs() -> u64 {
        Utc.with_ymd_and_hms(2024, 7, 15, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64
    }

    fn gmt_secs() -> u64 {
        Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64
    }

    #[test]
    fn bst_summer_23_to_24_does_not_match_19_56_utc() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = bst_secs();
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 71760),
            UNIX_EPOCH + Duration::from_secs(b + 72150),
            &s,
            tz()
        ));
    }

    #[test]
    fn bst_summer_23_to_24_matches_21_56_utc() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = bst_secs();
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 78960),
            UNIX_EPOCH + Duration::from_secs(b + 79350),
            &s,
            tz()
        ));
    }

    #[test]
    fn gmt_winter_23_to_24_does_not_match_19_56_utc() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = gmt_secs();
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 71760),
            UNIX_EPOCH + Duration::from_secs(b + 72150),
            &s,
            tz()
        ));
    }

    #[test]
    fn gmt_winter_23_to_24_does_not_match_20_56_utc_but_matches_22_56() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = gmt_secs();
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 75360),
            UNIX_EPOCH + Duration::from_secs(b + 75720),
            &s,
            tz()
        ));
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 82560),
            UNIX_EPOCH + Duration::from_secs(b + 82920),
            &s,
            tz()
        ));
    }

    #[test]
    fn same_utc_window_matches_bst_but_not_gmt() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        // Window at 21:56-22:02 UTC = 22:56-23:02 BST — overlaps 23:00-24:00 BST
        let ws = 78960u64;
        let we = 79320u64;
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(bst_secs() + ws),
            UNIX_EPOCH + Duration::from_secs(bst_secs() + we),
            &s,
            tz()
        ));
        // Same UTC window = 21:56-22:02 GMT — does NOT overlap 23:00-24:00 GMT
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + ws),
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + we),
            &s,
            tz()
        ));
    }

    #[test]
    fn partial_overlap_works_correctly() {
        let s = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = bst_secs();
        // 22:00-23:05 UTC = 23:00-00:05 BST — overlaps 23:00-24:00
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 79200),
            UNIX_EPOCH + Duration::from_secs(b + 83100),
            &s,
            tz()
        ));
        // 22:00-22:30 UTC = 23:00-23:30 BST — entirely inside
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 79200),
            UNIX_EPOCH + Duration::from_secs(b + 81000),
            &s,
            tz()
        ));
        // 20:00-20:30 UTC = 21:00-21:30 BST — entirely before
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 72000),
            UNIX_EPOCH + Duration::from_secs(b + 73800),
            &s,
            tz()
        ));
    }

    #[test]
    fn day_of_week_filtering_works() {
        let s = vec![ScheduleEntry {
            day_of_week: Some(1),
            start_sec: 82800,
            end_sec: 86400,
        }];
        let b = gmt_secs();
        // Jan 15, 2024 = Monday: (day+4)%7 = 1, dayOfWeek(1) == 1 → should match
        let win_s = 82860u64;
        let win_e = 82920u64;
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + win_s),
            UNIX_EPOCH + Duration::from_secs(b + win_e),
            &s,
            tz()
        ));
        // Jan 16, 2024 = Tuesday: (day+4)%7 = 2, dayOfWeek(1) != 2 → should NOT match
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs((b + 1 * 86400) + win_s),
            UNIX_EPOCH + Duration::from_secs((b + 1 * 86400) + win_e),
            &s,
            tz()
        ));
    }

    #[test]
    fn fish_info_includes_intuition_and_mooch_requirements() {
        let data = carbuncledata::carbuncle_fishes().unwrap();
        let warden = data.fish_by_id(24994).unwrap();
        let warden_json = serde_json::to_value(fish_to_info(warden, &data)).unwrap();
        assert_eq!(
            warden_json["intuitionRequirements"],
            serde_json::json!([
                {"amount": 3, "fish": "Indigo Prismfish"},
                {"amount": 3, "fish": "Firelight Goldfish"},
                {"amount": 5, "fish": "Green Prismfish"}
            ])
        );
        assert_eq!(warden_json["intuitionLengthSeconds"], 175);
        assert_eq!(warden_json["fishEyes"], false);
        assert_eq!(warden_json["bigFish"], true);
        assert_eq!(warden_json["bait"], "Stonefly Larva");
        assert_eq!(warden_json["moochPath"], serde_json::json!([]));
        assert!(warden_json["lure"].is_null());
        assert_eq!(warden_json["lureProc"], false);

        let mooching_fish = data.fish_by_id(4904).unwrap();
        let mooching_json = serde_json::to_value(fish_to_info(mooching_fish, &data)).unwrap();
        assert_eq!(mooching_json["bait"], "Lugworm");
        assert_eq!(
            mooching_json["moochPath"],
            serde_json::json!(["Merlthor Goby"])
        );

        let big_fish = data.fish_by_id(7678).unwrap();
        let big_fish_json = serde_json::to_value(fish_to_info(big_fish, &data)).unwrap();
        assert_eq!(big_fish_json["bigFish"], true);

        let shonisaurus = data.fish_by_id(8772).unwrap();
        let shonisaurus_json = serde_json::to_value(fish_to_info(shonisaurus, &data)).unwrap();
        assert_eq!(shonisaurus_json["bait"], "Hoverworm");
        assert_eq!(
            shonisaurus_json["moochPath"],
            serde_json::json!(["Cloud Cutter", "Mahar"])
        );

        let lure_fish = data.fish_by_id(43685).unwrap();
        let lure_json = serde_json::to_value(fish_to_info(lure_fish, &data)).unwrap();
        assert_eq!(lure_json["lure"], "Modest");
        assert_eq!(lure_json["lureProc"], true);
        assert_eq!(lure_json["snagging"], false);
        assert_eq!(lure_json["tug"], "!");
        assert_eq!(lure_json["hookset"], "Precision");

        let shined_copper_shark = data.fish_by_id(52006).unwrap();
        let shined_json = serde_json::to_value(fish_to_info(shined_copper_shark, &data)).unwrap();
        assert!(shined_json["lure"].is_null());
        assert_eq!(shined_json["lureProc"], false);
    }
}
