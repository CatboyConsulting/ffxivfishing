use std::cell::OnceCell;

use chrono_tz::Tz;
use ffxivfishing::{
    carbuncledata,
    eorzea_time::{EORZEA_SUN, EorzeaTime},
    fish::{DEFAULT_INTUITION_LOOKBACK_MINUTES, Fish, FishData},
    schedule::{self, ScheduleEntry},
};
use serde::Serialize;
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
    fish_eyes: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    intuition: Option<IntuitionWindowInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntuitionWindowInfo {
    prerequisite_windows: Vec<IntuitionWindowSetupInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntuitionWindowSetupInfo {
    amount: u8,
    fish_id: u32,
    fish: String,
    fish_eyes: bool,
    start_esec: u64,
    end_esec: u64,
    start_display: String,
    end_display: String,
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

fn fish_window_to_info(window: &ffxivfishing::fish::FishWindow, fd: &FishData) -> FishWindow {
    let item_name = |id: u32| {
        fd.item_by_id(id)
            .map(|item| item.name().to_string())
            .unwrap_or_else(|| format!("Unknown item ({id})"))
    };
    let intuition = window.intuition().map(|intuition| IntuitionWindowInfo {
        prerequisite_windows: intuition
            .prerequisite_windows()
            .iter()
            .map(|setup| IntuitionWindowSetupInfo {
                amount: setup.amount(),
                fish_id: setup.fish(),
                fish: item_name(setup.fish()),
                fish_eyes: setup.uses_fish_eyes(),
                start_esec: setup.window().start().as_esecs(),
                end_esec: setup.window().end().as_esecs(),
                start_display: setup.window().start().to_string(),
                end_display: setup.window().end().to_string(),
            })
            .collect(),
    });
    FishWindow {
        start_esec: window.start().as_esecs(),
        end_esec: window.end().as_esecs(),
        start_display: window.start().to_string(),
        end_display: window.end().to_string(),
        duration_esec: window.duration().total_seconds(),
        fish_eyes: window.uses_fish_eyes(),
        intuition,
    }
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
        let window = fish.next_window_with_fish_eyes(
            eorzea_time,
            true,
            filter_intuition,
            use_fish_eyes,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
            max_lookahead,
        );
        match window {
            Some(fw) => serde_json::to_string(&Some(fish_window_to_info(&fw, fd))),
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
        let windows: Vec<FishWindow> = fish
            .next_windows_with_fish_eyes(
                eorzea_time,
                limit,
                filter_intuition,
                use_fish_eyes,
                include_ongoing,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
            )
            .into_iter()
            .map(|window| fish_window_to_info(&window, fd))
            .collect();
        serde_json::to_string(&windows).map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

#[wasm_bindgen]
pub fn get_fish_windows_in_schedule(
    fish_id: u32,
    timestamp_esec: u64,
    schedule_json: &str,
    timeperiod_secs: u64,
    limit: u32,
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
        let windows: Vec<FishWindow> = schedule::fish_windows_in_schedule(
            fish,
            now_et,
            &local_schedule,
            timeperiod_secs,
            limit,
            tz,
            filter_intuition,
            use_fish_eyes,
            include_ongoing,
        )
        .into_iter()
        .map(|window| fish_window_to_info(&window, fd))
        .collect();

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
    et.unix_secs()
}

#[wasm_bindgen]
pub fn unix_to_eorzea_esec(unix_secs: u64) -> u64 {
    EorzeaTime::from_unix_secs(unix_secs).as_esecs()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn intuition_window_includes_fish_eyes_tags() {
        let data = carbuncledata::carbuncle_fishes().unwrap();
        let aquamaton = data.fish_by_id(33240).unwrap();
        let window = aquamaton
            .next_windows_with_fish_eyes(
                EorzeaTime::from_esecs(0),
                100,
                true,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
            )
            .into_iter()
            .find(|window| {
                window.intuition().is_some_and(|intuition| {
                    intuition
                        .prerequisite_windows()
                        .iter()
                        .any(|setup| setup.uses_fish_eyes())
                })
            })
            .expect("Aquamaton should have a Fish Eyes prerequisite window");
        let json = serde_json::to_value(fish_window_to_info(&window, &data)).unwrap();

        assert_eq!(json["fishEyes"], false);
        assert!(
            json["intuition"]["prerequisiteWindows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|setup| setup["fishEyes"] == true)
        );
    }

    #[test]
    fn intuition_window_includes_prerequisite_windows() {
        let data = carbuncledata::carbuncle_fishes().unwrap();
        let warden = data.fish_by_id(24994).unwrap();
        let window = warden
            .next_windows_with_fish_eyes(
                EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(),
                10,
                true,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
            )
            .into_iter()
            .next()
            .expect("Warden of the Seven Hues should have an intuition window");
        let json = serde_json::to_value(fish_window_to_info(&window, &data)).unwrap();

        assert_eq!(
            json["intuition"]["prerequisiteWindows"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            json["intuition"]["prerequisiteWindows"][0]["fish"],
            "Indigo Prismfish"
        );
        assert_eq!(json["intuition"]["prerequisiteWindows"][0]["amount"], 3);

        let fish_eyes_window = warden
            .next_windows_with_fish_eyes(
                EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(),
                10,
                true,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
            )
            .into_iter()
            .next()
            .expect("Warden should have a Fish Eyes intuition window");
        let fish_eyes_json =
            serde_json::to_value(fish_window_to_info(&fish_eyes_window, &data)).unwrap();
        assert_eq!(
            fish_eyes_json["intuition"]["prerequisiteWindows"][0]["startDisplay"],
            "0001-01-02 00:00:00"
        );
        assert_eq!(
            fish_eyes_json["intuition"]["prerequisiteWindows"][0]["endDisplay"],
            "0001-01-02 04:00:00"
        );
    }

    #[test]
    fn sidereal_whale_has_windows_with_fish_eyes_enabled() {
        let data = carbuncledata::carbuncle_fishes().unwrap();
        let whale = data.fish_by_id(41412).unwrap();
        let windows = whale.next_windows_with_fish_eyes(
            EorzeaTime::from_esecs(0),
            100,
            true,
            true,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );
        assert_eq!(windows.len(), 1);
    }
}
