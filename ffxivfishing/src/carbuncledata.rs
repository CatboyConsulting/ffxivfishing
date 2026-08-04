use std::collections::{HashMap, HashSet};
use std::{error::Error, rc::Rc, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    eorzea_time::EorzeaDuration,
    fish::{Bait, Fish, FishData, FishingHole, FishingItem, Intuition, Lure, Region},
    weather::{Weather, WeatherForecast},
};

const DATA: &str = include_str!("data.json");

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum OneOrVec<T> {
    One(T),
    Vec(Vec<T>),
}

#[derive(Debug, Serialize, Deserialize)]
struct CarbuncleZone {
    #[serde(rename = "name_en")]
    name_en: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CarbuncleRegion {
    #[serde(rename = "name_en")]
    name_en: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CarbuncleData {
    #[serde(rename = "FISH")]
    fishes: HashMap<String, CarbuncleFish>,
    #[serde(rename = "WEATHER_RATES")]
    weather_rates: HashMap<String, CarbuncleWeatherRates>,
    #[serde(rename = "FISHING_SPOTS")]
    fishing_spots: HashMap<String, CarbuncleFishingSpot>,
    #[serde(rename = "ITEMS")]
    items: HashMap<String, CarbuncleItem>,
    #[serde(rename = "ZONES")]
    zones: HashMap<String, CarbuncleZone>,
    #[serde(rename = "REGIONS")]
    regions: HashMap<String, CarbuncleRegion>,
    #[serde(rename = "WEATHER_TYPES")]
    weather_types: HashMap<String, CarbuncleWeatherType>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CarbuncleWeatherType {
    #[serde(rename = "name_en")]
    name_en: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CarbuncleFish {
    #[serde(rename = "_id")]
    id: u32,
    #[serde(rename = "previousWeatherSet")]
    previous_weather_set: Vec<u32>,
    #[serde(rename = "weatherSet")]
    weather_set: Vec<u32>,
    #[serde(rename = "bestCatchPath")]
    best_catch_path: Vec<OneOrVec<u32>>,
    #[serde(rename = "startHour")]
    start_hour: f32,
    #[serde(rename = "endHour")]
    end_hour: f32,
    #[serde(rename = "location")]
    location: Option<u32>,
    #[serde(rename = "intuitionLength")]
    intuition_length: Option<u32>,
    #[serde(rename = "predators")]
    predators: Vec<[u32; 2]>,
    #[serde(rename = "tug")]
    tug: Option<String>,
    #[serde(rename = "hookset")]
    hookset: Option<String>,
    #[serde(rename = "lure")]
    lure: Option<String>,
    #[serde(rename = "fishEyes")]
    fish_eyes: bool,
    #[serde(rename = "bigFish")]
    big_fish: bool,
    #[serde(rename = "snagging")]
    snagging: Option<bool>,
    #[serde(rename = "patch")]
    patch: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct CarbuncleFishingSpot {
    #[serde(rename = "_id")]
    id: u32,
    #[serde(rename = "name_en")]
    name: String,
    #[serde(rename = "map_coords")]
    map_coords: [f32; 3],
    #[serde(rename = "territory_id")]
    territory_id: u32,
    #[serde(rename = "placename_id")]
    placename_id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct CarbuncleItem {
    #[serde(rename = "_id")]
    id: u32,
    #[serde(rename = "name_en")]
    name: String,
    #[serde(rename = "icon")]
    icon: String,
    #[serde(rename = "ilvl")]
    ilvl: u32,
}
impl CarbuncleItem {
    fn to_fishing_item(&self, fishes: &[Fish]) -> FishingItem {
        match fishes.iter().find(|f| f.id == self.id) {
            Some(f) => FishingItem::Fish(self.name.clone(), f.id),
            None => FishingItem::Bait(self.name.clone(), self.id),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CarbuncleWeatherRates {
    #[serde(rename = "map_id")]
    map_id: u32,
    #[serde(rename = "map_scale")]
    map_scale: u32,
    #[serde(rename = "zone_id")]
    zone_id: u32,
    #[serde(rename = "region_id")]
    region_id: u32,
    #[serde(rename = "weather_rates")]
    weather_rates: Vec<(u32, u8)>,
}

impl From<&CarbuncleWeatherRates> for WeatherForecast {
    fn from(cwr: &CarbuncleWeatherRates) -> Self {
        WeatherForecast::new(
            cwr.map_id.to_string(),
            cwr.weather_rates
                .iter()
                .map(|(weather_id, rate)| (*rate, Weather::Id(*weather_id)))
                .collect(),
        )
    }
}

impl CarbuncleFishingSpot {
    fn to_fishinghole(
        &self,
        regions: &[Rc<Region>],
        wr_key_to_idx: &HashMap<String, usize>,
    ) -> Option<FishingHole> {
        let idx = wr_key_to_idx.get(&self.territory_id.to_string())?;
        let region = regions.get(*idx)?.clone();
        Some(FishingHole::new(self.id, self.name.clone(), region))
    }
}

impl CarbuncleFish {
    fn lure_type(&self) -> Lure {
        match self.lure.as_deref() {
            Some(value) if value.eq_ignore_ascii_case("Ambitious") => Lure::Ambitious,
            _ => Lure::Modest,
        }
    }

    fn try_get_intuition(&self) -> Option<Intuition> {
        if self.intuition_length.is_none() && self.predators.is_empty() {
            return None;
        }

        let requirements = self.predators.iter().map(|p| (p[1] as u8, p[0])).collect();
        Some(match self.intuition_length {
            Some(length) => Intuition::new(Duration::from_secs(length as u64), requirements),
            None => Intuition::without_length(requirements),
        })
    }

    fn to_fish(
        &self,
        fishing_holes: &[Rc<FishingHole>],
        items: &[&CarbuncleItem],
        fish_ids: &HashSet<u32>,
    ) -> Option<Fish> {
        let loc = self.location?;
        let fish_hole = fishing_holes.iter().find(|fh| fh.id() == loc)?;
        let item = items.iter().find(|i| self.id == i.id)?;

        let catch_path: Vec<u32> = self
            .best_catch_path
            .iter()
            .filter_map(|path| match path {
                OneOrVec::One(id) => Some(*id),
                OneOrVec::Vec(ids) => ids.last().copied(),
            })
            .collect();
        let first_fish = catch_path.iter().position(|id| fish_ids.contains(id));
        let bait = match (catch_path.first().copied(), first_fish) {
            (Some(bait_id), Some(first_fish)) => Bait::Mooch {
                bait_id: (first_fish > 0).then_some(bait_id),
                fish_ids: catch_path[first_fish..].to_vec(),
            },
            (_, None) => catch_path
                .last()
                .copied()
                .map(Bait::Bait)
                .unwrap_or(Bait::Unknown),
            (None, _) => Bait::Unknown,
        };
        Some(Fish::new(
            self.id,
            item.name.clone(),
            Rc::clone(fish_hole),
            EorzeaDuration::from_esecs((self.start_hour * 3600.0) as u64),
            EorzeaDuration::from_esecs((self.end_hour * 3600.0) as u64),
            bait,
            self.previous_weather_set
                .iter()
                .map(|id| Weather::Id(*id))
                .collect(),
            self.weather_set.iter().map(|id| Weather::Id(*id)).collect(),
            self.tug.clone().unwrap_or("".to_string()).as_str().into(),
            self.hookset
                .clone()
                .unwrap_or("".to_string())
                .as_str()
                .into(),
            self.try_get_intuition(),
            self.lure_type(),
            self.lure.is_some(),
            self.snagging.unwrap_or(false),
            false,
            false,
            self.fish_eyes,
            self.big_fish,
            (
                ((self.patch * 100.0).round() as u16 / 100) as u8,
                ((self.patch * 100.0).round() as u16 % 100) as u8,
            ),
        ))
    }
}

fn parse_fishes() -> Result<Vec<CarbuncleFish>, serde_json::Error> {
    let data: serde_json::Value = serde_json::from_str(DATA)?;

    let fishes = match data["FISH"].as_object() {
        Some(f) => f.clone(),
        None => return Ok(vec![]),
    };

    Ok(fishes
        .values()
        .filter_map(|f| serde_json::from_value::<CarbuncleFish>(f.clone()).ok())
        .collect())
}

fn parse_fishing_spots() -> Result<Vec<CarbuncleFishingSpot>, serde_json::Error> {
    let data: serde_json::Value = serde_json::from_str(DATA)?;

    let fish_spots = match data["FISHING_SPOTS"].as_object() {
        Some(f) => f.clone(),
        None => return Ok(vec![]),
    };

    Ok(fish_spots
        .values()
        .filter_map(|f| serde_json::from_value::<CarbuncleFishingSpot>(f.clone()).ok())
        .collect())
}

fn parse_weather() -> Result<Vec<CarbuncleWeatherRates>, serde_json::Error> {
    let data: serde_json::Value = serde_json::from_str(DATA)?;

    let fishes = match data["WEATHER_RATES"].as_object() {
        Some(f) => f.clone(),
        None => return Ok(vec![]),
    };

    Ok(fishes
        .values()
        .filter_map(|f| serde_json::from_value::<CarbuncleWeatherRates>(f.clone()).ok())
        .collect())
}

fn parse_data() -> Result<CarbuncleData, serde_json::Error> {
    serde_json::from_str(DATA)
}

impl CarbuncleData {
    fn convert_to_fishdata(&self) -> FishData {
        let weather_names: HashMap<u32, String> = self
            .weather_types
            .iter()
            .map(|(id, wt)| (id.parse().unwrap_or(0), wt.name_en.clone()))
            .collect();
        let weather_rates: HashMap<String, WeatherForecast> = self
            .weather_rates
            .clone()
            .into_iter()
            .map(|(id, w)| (id, (&w).into()))
            .collect();

        let items: Vec<&CarbuncleItem> = self.items.values().collect();
        let fish_ids: HashSet<u32> = self.fishes.values().map(|fish| fish.id).collect();

        let wr_key_to_idx: HashMap<String, usize> = weather_rates
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect();

        let regions: Vec<Rc<Region>> = weather_rates
            .iter()
            .map(|(id, wf)| {
                let zone_name = self
                    .weather_rates
                    .get(id)
                    .and_then(|cwr| self.zones.get(&cwr.zone_id.to_string()))
                    .map(|z| z.name_en.clone())
                    .unwrap_or_else(|| id.clone());
                Rc::new(Region::new(zone_name, wf.clone()))
            })
            .collect();

        let fishing_holes: Vec<Rc<FishingHole>> = self
            .fishing_spots
            .values()
            .filter_map(|fs| fs.to_fishinghole(&regions, &wr_key_to_idx))
            .map(Rc::new)
            .collect();

        let fishes: Vec<Fish> = self
            .fishes
            .values()
            .filter_map(|f| f.to_fish(&fishing_holes, &items, &fish_ids))
            .collect();
        let fishing_items = items
            .iter()
            .map(|item| item.to_fishing_item(&fishes))
            .collect();
        FishData::new(fishes, fishing_holes, regions, fishing_items, weather_names)
    }
}

pub fn carbuncle_fishes_from_str(data: &str) -> Result<FishData, Box<dyn Error>> {
    let parsed: CarbuncleData = serde_json::from_str(data)?;
    Ok(parsed.convert_to_fishdata())
}

pub fn carbuncle_fishes() -> Result<FishData, Box<dyn Error>> {
    carbuncle_fishes_from_str(DATA)
}

#[cfg(test)]
mod tests {

    use std::time::SystemTime;

    use crate::{
        eorzea_time::{EORZEA_SUN, EorzeaTime},
        fish::DEFAULT_INTUITION_LOOKBACK_MINUTES,
    };

    use super::*;
    #[test]
    fn parse_fishing_spots_test() {
        let fish_spots = parse_fishing_spots().unwrap();
        assert!(!fish_spots.is_empty());
        for s in fish_spots {
            println!("{}", s.territory_id);
        }
    }

    #[test]
    fn weather_at() {
        let weathers = parse_weather().unwrap();
        assert!(!weathers.is_empty());
        for w in weathers {
            let eorzea_weather: WeatherForecast = (&w).into();
            let _ = eorzea_weather.weather_at(EorzeaTime::from_time(&SystemTime::now()).unwrap());
        }
    }

    #[test]
    fn fish_location_names() {
        let data = parse_data().unwrap();
        let fishes = data.convert_to_fishdata();
        let mut numeric_loc = 0;
        let mut numeric_reg = 0;
        for fish in fishes.fishes().iter().take(10) {
            let loc = fish.location.name();
            let reg = fish.location.region().name();
            println!("fish={}, location='{}', region='{}'", fish.name, loc, reg);
            if loc.chars().all(|c| c.is_ascii_digit()) {
                numeric_loc += 1;
            }
            if reg.chars().all(|c| c.is_ascii_digit()) {
                numeric_reg += 1;
            }
        }
        for fish in fishes.fishes() {
            if fish.location.name().chars().all(|c| c.is_ascii_digit()) {
                numeric_loc += 1;
            }
            if fish
                .location
                .region()
                .name()
                .chars()
                .all(|c| c.is_ascii_digit())
            {
                numeric_reg += 1;
            }
        }
        println!("Fish with numeric locations: {}", numeric_loc);
        println!("Fish with numeric regions: {}", numeric_reg);
        assert!(numeric_loc < 10, "Too many numeric locations");
        assert!(numeric_reg < 10, "Too many numeric regions");
    }

    #[test]
    fn parse_data_test() {
        let data = parse_data().unwrap();
        let fishes = data.convert_to_fishdata();
        for fish in fishes.fishes() {
            let window = fish.next_window(
                EorzeaTime::from_time(&SystemTime::now()).unwrap(),
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                1_000,
            );
            match window {
                Some(ref _window1) => {
                    let w = window.unwrap();
                    println!(
                        "{:?}: {} - {:?}",
                        fish.name(),
                        w,
                        w.start().to_system_time()
                    );
                }
                None => {
                    println!("{:?}: !!!", fish.name());
                }
            }
        }
    }

    #[test]
    fn warden_of_the_seven_hues_intuition_windows() {
        let data = carbuncle_fishes().unwrap();
        let fish = data.fish_by_id(24994).unwrap();
        let first = fish
            .next_window(
                EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(),
                false,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                10_000,
            )
            .unwrap();
        assert_eq!(first.start(), EorzeaTime::new(1, 1, 2, 8, 0, 0).unwrap());
        assert_eq!(first.end(), EorzeaTime::new(1, 1, 2, 17, 0, 0).unwrap());

        let mut current = EorzeaTime::now();

        for _ in 1..=10 {
            let window = fish
                .next_window(
                    current,
                    false,
                    true,
                    false,
                    DEFAULT_INTUITION_LOOKBACK_MINUTES,
                    10_000,
                )
                .expect("missing Warden window");
            assert!(window.duration().total_seconds() < EORZEA_SUN.total_seconds());
            current = window.end();
        }
    }

    #[test]
    fn cinder_surprise_finds_windows_after_the_first_one() {
        let data = carbuncle_fishes().unwrap();
        let fish = data.fish_by_id(33241).unwrap();
        let first = fish
            .next_window(
                EorzeaTime::new(1108, 8, 15, 0, 0, 0).unwrap(),
                true,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                10_000,
            )
            .unwrap();
        assert_eq!(
            first.start(),
            EorzeaTime::new(1108, 8, 15, 0, 0, 0).unwrap()
        );

        let next = fish
            .next_window(
                first.end(),
                true,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                10_000,
            )
            .expect("missing Cinder Surprise window after the first one");
        assert!(next.start() > first.end());
    }

    #[test]
    fn requirement_metadata_is_resolved() {
        let data = carbuncle_fishes().unwrap();
        let warden = data.fish_by_id(24994).unwrap();
        assert_eq!(
            warden.intuition_requirements(),
            Some(&[(3, 24203), (3, 23056), (5, 24204)][..])
        );

        let mooching_fish = data.fish_by_id(4904).unwrap();
        assert_eq!(mooching_fish.mooch_id(), Some(4869));
    }
}
