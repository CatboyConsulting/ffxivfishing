use std::{
    collections::HashMap,
    fmt::Display,
    rc::Rc,
    time::{Duration, SystemTime},
};

use crate::{
    eorzea_time::{EORZEA_SUN, EORZEA_WEATHER_PERIOD, EorzeaDuration, EorzeaTime, EorzeaTimeSpan},
    weather::{Weather, WeatherForecast},
};

#[derive(Debug, Clone)]
pub struct Region {
    name: String,
    weather: WeatherForecast,
}

#[derive(Debug)]
pub struct FishingHole {
    id: u32,
    name: String,
    region: Rc<Region>,
}

#[derive(Debug)]
pub enum Tug {
    Light,
    Medium,
    Heavy,
    Unknown,
}

impl From<&str> for Tug {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "light" => Tug::Light,
            "medium" => Tug::Medium,
            "heavy" => Tug::Heavy,
            _ => Tug::Unknown,
        }
    }
}

impl Display for Tug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Tug::Light => "!",
                Tug::Medium => "!!",
                Tug::Heavy => "!!!",
                Tug::Unknown => "?",
            }
        )
    }
}

#[derive(Debug)]
pub enum Hookset {
    Precision,
    Powerful,
    Unknown,
}
impl From<&str> for Hookset {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "precision" => Hookset::Precision,
            "powerful" => Hookset::Powerful,
            _ => Hookset::Unknown,
        }
    }
}

impl Display for Hookset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Hookset::Precision => "Precision",
                Hookset::Powerful => "Powerful",
                Hookset::Unknown => "Unknown",
            }
        )
    }
}

#[derive(Debug)]
pub enum Bait {
    Mooch {
        bait_id: Option<u32>,
        fish_ids: Vec<u32>,
    },
    Bait(u32),
    Unknown,
}

#[derive(Debug, Clone)]
struct FishWindowDefinition {
    location: Rc<FishingHole>,
    window_start: EorzeaDuration,
    window_end: EorzeaDuration,
    previous_weather_set: Vec<Weather>,
    weather_set: Vec<Weather>,
}

impl FishWindowDefinition {
    fn window_on_day(&self, etime: EorzeaTime) -> EorzeaTimeSpan {
        let mut day = etime;
        day.round(EORZEA_SUN);
        let start = day + self.window_start;
        let mut end = day + self.window_end;
        if end <= start {
            end += EORZEA_SUN;
        }
        EorzeaTimeSpan::new_start_end(start, end).unwrap()
    }

    fn next_window(
        &self,
        start: EorzeaTime,
        include_ongoing: bool,
        mut limit: u32,
    ) -> Option<EorzeaTimeSpan> {
        // A fish with no weather restrictions is available for its complete
        // time window, rather than only for each weather period in that window.
        if self.previous_weather_set.is_empty() && self.weather_set.is_empty() {
            let mut time = start;
            while limit > 0 {
                let window = self.window_on_day(time);
                let valid = if include_ongoing {
                    window.end() > start
                } else {
                    window.start() >= start
                };
                if valid && window.duration().total_seconds() > 0 {
                    return Some(window);
                }
                time += EORZEA_SUN;
                limit -= 1;
            }
            return None;
        }

        let mut time = start;
        while limit > 0 {
            let next_weather = self.location.region.weather.find_pattern(
                time,
                &self.previous_weather_set,
                &self.weather_set,
                limit,
            )?;
            let weather_span = EorzeaTimeSpan::new(next_weather, EORZEA_WEATHER_PERIOD);
            if let Ok(window) = self.window_on_day(time).overlap(&weather_span) {
                let min_window = match include_ongoing {
                    true => window.end(),
                    false => window.start(),
                };
                let valid = if include_ongoing {
                    start < min_window
                } else {
                    start <= min_window
                };
                if valid && window.duration().total_seconds() > 0 {
                    let mut merged = window;
                    let mut next_time = merged.end();
                    let mut remaining = limit.saturating_sub(1);
                    while remaining > 0 {
                        let next_weather = match self.location.region.weather.find_pattern(
                            next_time,
                            &self.previous_weather_set,
                            &self.weather_set,
                            remaining,
                        ) {
                            Some(next_weather) => next_weather,
                            None => break,
                        };
                        let next_weather_span =
                            EorzeaTimeSpan::new(next_weather, EORZEA_WEATHER_PERIOD);
                        let next_window =
                            match self.window_on_day(next_time).overlap(&next_weather_span) {
                                Ok(next_window) => next_window,
                                Err(_) => break,
                            };
                        if next_window.start() != merged.end()
                            || next_window.duration().total_seconds() == 0
                        {
                            break;
                        }
                        merged = EorzeaTimeSpan::new_start_end(merged.start(), next_window.end())
                            .ok()?;
                        next_time = merged.end();
                        remaining -= 1;
                    }
                    return Some(merged);
                }
            }
            time += EORZEA_WEATHER_PERIOD;
            limit -= 1;
        }
        None
    }

    fn last_window_in(&self, start: EorzeaTime, end: EorzeaTime) -> Option<EorzeaTimeSpan> {
        if start >= end {
            return None;
        }

        let mut time = start;
        let mut limit = INTUITION_SEARCH_LIMIT;
        let mut last_window = None;
        while limit > 0 {
            let window = match self.next_window(time, true, limit) {
                Some(window) => window,
                None => return last_window,
            };
            if window.start() < end && window.end() > start {
                last_window = Some(window.clone());
            }
            if window.start() >= end {
                return last_window;
            }

            let next_time = window.end();
            if next_time <= time {
                return last_window;
            }
            if next_time >= end {
                return last_window;
            }
            time = next_time;
            limit -= 1;
        }
        last_window
    }
}

#[derive(Debug)]
pub struct Intuition {
    length: Option<Duration>,
    requirements: Vec<(u8, u32)>,
    resolved_requirements: Option<Vec<(u8, Option<FishWindowDefinition>)>>,
}
impl Intuition {
    pub(crate) fn new(length: Duration, requirements: Vec<(u8, u32)>) -> Self {
        Self {
            length: Some(length),
            requirements,
            resolved_requirements: None,
        }
    }

    pub(crate) fn without_length(requirements: Vec<(u8, u32)>) -> Self {
        Self {
            length: None,
            requirements,
            resolved_requirements: None,
        }
    }

    fn length_eorzea(&self) -> Option<EorzeaDuration> {
        self.length
            .map(|length| EorzeaDuration::from_esecs((length.as_secs() * 3_600 + 87) / 175))
    }
}

#[derive(Debug)]
pub enum Lure {
    Modest,
    Ambitious,
}

impl Display for Lure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Lure::Modest => "Modest",
            Lure::Ambitious => "Ambitious",
        })
    }
}

#[derive(Debug)]
pub struct Fish {
    pub id: u32,
    pub name: String,
    pub location: Rc<FishingHole>,
    pub window_start: EorzeaDuration,
    pub window_end: EorzeaDuration,
    pub bait: Bait,
    pub previous_weather_set: Vec<Weather>,
    pub weather_set: Vec<Weather>,
    pub tug: Tug,
    pub hookset: Hookset,
    pub intuition: Option<Intuition>,
    pub lure: Lure,
    pub lure_proc: bool,
    pub snagging: bool,
    pub gig: bool,
    pub folklore: bool,
    pub fish_eyes: bool,
    pub big_fish: bool,
    pub patch: (u8, u8),
}

pub const DEFAULT_INTUITION_LOOKBACK_MINUTES: u64 = 30;
const INTUITION_SEARCH_LIMIT: u32 = 10_000;

impl Fish {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        name: String,
        location: Rc<FishingHole>,
        window_start: EorzeaDuration,
        window_end: EorzeaDuration,
        bait: Bait,
        previous_weather_set: Vec<Weather>,
        weather_set: Vec<Weather>,
        tug: Tug,
        hookset: Hookset,
        intuition: Option<Intuition>,
        lure: Lure,
        lure_proc: bool,
        snagging: bool,
        gig: bool,
        folklore: bool,
        fish_eyes: bool,
        big_fish: bool,
        patch: (u8, u8),
    ) -> Fish {
        Self {
            id,
            name,
            location,
            window_start: window_start % EORZEA_SUN,
            window_end: window_end % EORZEA_SUN,
            bait,
            previous_weather_set,
            weather_set,
            tug,
            hookset,
            intuition,
            lure,
            lure_proc,
            snagging,
            gig,
            folklore,
            fish_eyes,
            big_fish,
            patch,
        }
    }

    pub fn window_on_day(&self, etime: EorzeaTime) -> EorzeaTimeSpan {
        self.window_definition(false).window_on_day(etime)
    }

    pub fn next_window(
        &self,
        start: EorzeaTime,
        include_ongoing: bool,
        filter_intuition: bool,
        use_fish_eyes: bool,
        intuition_lookback_minutes: u64,
        mut limit: u32,
    ) -> Option<EorzeaTimeSpan> {
        let definition = self.window_definition(use_fish_eyes);
        let mut time = start;
        while limit > 0 {
            let include_target_ongoing = include_ongoing
                || (filter_intuition
                    && self.intuition.is_some()
                    && (self.is_always_available() || (use_fish_eyes && self.fish_eyes)));
            let window = definition.next_window(time, include_target_ongoing, limit)?;
            if !filter_intuition {
                return Some(window);
            }
            if let Some(window) = self.intuition_window(&window, intuition_lookback_minutes) {
                if window.end() > time {
                    return Some(window);
                }
            }

            // Rejected candidates still consume search space. Advancing by at
            // least one weather period prevents an unavailable intuition fish
            // from causing an unbounded search.
            let elapsed = window.start().as_esecs().saturating_sub(time.as_esecs());
            let period = EORZEA_WEATHER_PERIOD.total_seconds();
            let consumed = (elapsed / period).max(1) as u32;
            if consumed >= limit {
                return None;
            }
            limit -= consumed;
            time = window.end();
        }
        None
    }

    fn window_definition(&self, use_fish_eyes: bool) -> FishWindowDefinition {
        let ignore_time = use_fish_eyes && self.fish_eyes;
        FishWindowDefinition {
            location: Rc::clone(&self.location),
            window_start: if ignore_time {
                EorzeaDuration::from_esecs(0)
            } else {
                self.window_start
            },
            window_end: if ignore_time {
                EorzeaDuration::from_esecs(0)
            } else {
                self.window_end
            },
            previous_weather_set: self.previous_weather_set.clone(),
            weather_set: self.weather_set.clone(),
        }
    }

    fn intuition_window(
        &self,
        window: &EorzeaTimeSpan,
        intuition_lookback_minutes: u64,
    ) -> Option<EorzeaTimeSpan> {
        let intuition = match &self.intuition {
            Some(intuition) => intuition,
            None => return Some(window.clone()),
        };
        let intuition_length = match intuition.length_eorzea() {
            Some(length) => length,
            None => return Some(window.clone()),
        };
        let requirements = match &intuition.resolved_requirements {
            Some(requirements) => requirements,
            None => return None,
        };

        let lookback_esecs = ((intuition_lookback_minutes as u128 * 60 * 3_600 + 87) / 175)
            .min(u64::MAX as u128) as u64;
        let lookback = EorzeaDuration::from_esecs(lookback_esecs);
        let preparation_start = window.start() - lookback;
        let preparation_end = window.end();

        let mut last_prerequisite = None;
        for (_, prerequisite) in requirements {
            let prerequisite = prerequisite.as_ref()?;
            let prerequisite_window =
                prerequisite.last_window_in(preparation_start, preparation_end)?;
            if last_prerequisite
                .as_ref()
                .is_none_or(|last: &EorzeaTimeSpan| prerequisite_window.end() > last.end())
            {
                last_prerequisite = Some(prerequisite_window);
            }
        }

        let last_prerequisite = last_prerequisite?;
        let intuition_end = last_prerequisite.end() + intuition_length;
        let start = std::cmp::max(window.start(), last_prerequisite.start());
        let end = std::cmp::min(window.end(), intuition_end);
        EorzeaTimeSpan::new_start_end(start, end).ok()
    }

    fn is_always_available(&self) -> bool {
        // Fish::new normalizes endHour 24 to zero, so a full-day window is
        // represented by equal zero-valued start and end durations.
        self.window_start == EorzeaDuration::from_esecs(0)
            && self.window_end == EorzeaDuration::from_esecs(0)
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn start(&self) -> &EorzeaDuration {
        &self.window_start
    }
    pub fn time_restriction(&self) -> (&EorzeaDuration, &EorzeaDuration) {
        (&self.window_start, &self.window_end)
    }

    pub fn weather_now(&self) -> &Weather {
        self.location
            .region
            .weather
            .weather_at(EorzeaTime::from_time(&SystemTime::now()).unwrap())
    }
    pub fn bait_id(&self) -> Option<u32> {
        match self.bait {
            Bait::Mooch { ref fish_ids, .. } => fish_ids.last().copied(),
            Bait::Bait(id) => Some(id),
            Bait::Unknown => None,
        }
    }

    pub fn base_bait_id(&self) -> Option<u32> {
        match self.bait {
            Bait::Mooch { bait_id, .. } => bait_id,
            Bait::Bait(id) => Some(id),
            Bait::Unknown => None,
        }
    }

    pub fn mooch_id(&self) -> Option<u32> {
        match self.bait {
            Bait::Mooch { ref fish_ids, .. } => fish_ids.last().copied(),
            Bait::Bait(_) | Bait::Unknown => None,
        }
    }

    pub fn mooch_path(&self) -> Option<&[u32]> {
        match &self.bait {
            Bait::Mooch { fish_ids, .. } => Some(fish_ids),
            Bait::Bait(_) | Bait::Unknown => None,
        }
    }

    pub fn intuition_requirements(&self) -> Option<&[(u8, u32)]> {
        self.intuition
            .as_ref()
            .map(|intuition| intuition.requirements.as_slice())
    }

    pub fn intuition_length_seconds(&self) -> Option<u64> {
        self.intuition
            .as_ref()
            .and_then(|intuition| intuition.length.map(|length| length.as_secs()))
    }
}

impl FishingHole {
    pub fn new(id: u32, name: String, region: Rc<Region>) -> FishingHole {
        FishingHole { id, name, region }
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn region(&self) -> &Rc<Region> {
        &self.region
    }
}

impl Region {
    pub fn new(name: String, weather: WeatherForecast) -> Region {
        Region { name, weather }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn weather(&self) -> &WeatherForecast {
        &self.weather
    }
}

#[derive(Debug, Clone)]
pub enum FishingItem {
    Fish(String, u32),
    Bait(String, u32),
}
impl FishingItem {
    pub fn name(&self) -> &str {
        match self {
            FishingItem::Fish(name, _) => name,
            FishingItem::Bait(name, _) => name,
        }
    }
    pub fn id(&self) -> u32 {
        match self {
            FishingItem::Fish(_, id) => *id,
            FishingItem::Bait(_, id) => *id,
        }
    }
}

pub struct FishData {
    fishes: Vec<Fish>,
    fishing_holes: Vec<Rc<FishingHole>>,
    regions: Vec<Rc<Region>>,
    items: Vec<FishingItem>,
    weather_names: HashMap<u32, String>,
}

impl FishData {
    pub fn new(
        mut fishes: Vec<Fish>,
        fishing_holes: Vec<Rc<FishingHole>>,
        regions: Vec<Rc<Region>>,
        items: Vec<FishingItem>,
        weather_names: HashMap<u32, String>,
    ) -> FishData {
        let definitions: HashMap<u32, FishWindowDefinition> = fishes
            .iter()
            .map(|fish| (fish.id, fish.window_definition(false)))
            .collect();
        for fish in &mut fishes {
            if let Some(intuition) = &mut fish.intuition {
                intuition.resolved_requirements = Some(
                    intuition
                        .requirements
                        .iter()
                        .map(|(count, id)| (*count, definitions.get(id).cloned()))
                        .collect(),
                );
            }
        }

        FishData {
            fishes,
            fishing_holes,
            regions,
            items,
            weather_names,
        }
    }

    pub fn weather_name(&self, w: &Weather) -> String {
        match w {
            Weather::Unknown => "Unknown".to_string(),
            Weather::Id(id) => self
                .weather_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("Id({})", id)),
            Weather::Sunny => "Sunny".to_string(),
            Weather::Clouds => "Clouds".to_string(),
            Weather::ClearSkies => "Clear Skies".to_string(),
            Weather::FairSkies => "Fair Skies".to_string(),
            Weather::Fog => "Fog".to_string(),
            Weather::Wind => "Wind".to_string(),
        }
    }
    pub fn item_by_id(&self, id: u32) -> Option<&FishingItem> {
        self.items.iter().find(|item| item.id() == id)
    }
    pub fn fish_by_id(&self, id: u32) -> Option<&Fish> {
        self.fishes.iter().find(|f| f.id == id)
    }

    pub fn fishes(&self) -> &Vec<Fish> {
        &self.fishes
    }

    pub fn search_fish(&self, query: &str) -> Vec<(u32, String)> {
        let query_lower = query.to_lowercase();
        self.fishes
            .iter()
            .filter(|f| f.name.to_lowercase().contains(&query_lower))
            .map(|f| (f.id, f.name.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    pub fn next_window() {
        let weather = WeatherForecast::new(
            "Region".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let fishing_hole = FishingHole {
            id: 0,
            name: "Fishing Hole".to_string(),
            region: Rc::new(Region {
                name: "Region".to_string(),
                weather,
            }),
        };
        let fish = Fish {
            id: 0,
            name: "".to_string(),
            location: Rc::new(fishing_hole),
            window_start: EorzeaDuration::new(1, 0, 0).unwrap(),
            window_end: EorzeaDuration::new(2, 0, 0).unwrap(),
            bait: Bait::Bait(0),
            previous_weather_set: vec![Weather::Clouds],
            weather_set: vec![Weather::Clouds],
            tug: Tug::Light,
            hookset: Hookset::Precision,
            intuition: None,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            patch: (7, 0),
            lure: Lure::Modest,
            lure_proc: false,
        };
        let result = fish
            .next_window(
                EorzeaTime::new(1, 1, 2, 2, 0, 0).unwrap(),
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                1000,
            )
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 3, 1, 0, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 3, 2, 0, 0).unwrap());
    }

    #[test]
    pub fn next_window_includes_ongoing_window() {
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new(
                "Region".to_string(),
                WeatherForecast::new("Region".to_string(), vec![]),
            )),
        ));
        let fish = Fish::new(
            0,
            "".to_string(),
            location,
            EorzeaDuration::new(1, 0, 0).unwrap(),
            EorzeaDuration::new(2, 0, 0).unwrap(),
            Bait::Unknown,
            vec![],
            vec![],
            Tug::Unknown,
            Hookset::Unknown,
            None,
            Lure::Modest,
            false,
            false,
            false,
            false,
            false,
            false,
            (1, 0),
        );
        let now = EorzeaTime::new(1, 1, 1, 1, 30, 0).unwrap();

        let ongoing = fish
            .next_window(
                now,
                true,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(ongoing.start(), EorzeaTime::new(1, 1, 1, 1, 0, 0).unwrap());
        assert_eq!(ongoing.end(), EorzeaTime::new(1, 1, 1, 2, 0, 0).unwrap());

        let upcoming = fish
            .next_window(
                now,
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(upcoming.start(), EorzeaTime::new(1, 1, 2, 1, 0, 0).unwrap());
    }

    #[test]
    pub fn next_window_weather_border() {
        let weather = WeatherForecast::new(
            "Region".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let fishing_hole = FishingHole {
            id: 0,
            name: "Fishing Hole".to_string(),
            region: Rc::new(Region {
                name: "Region".to_string(),
                weather,
            }),
        };
        let fish = Fish {
            id: 0,
            name: "".to_string(),
            location: Rc::new(fishing_hole),
            window_start: EorzeaDuration::new(7, 30, 0).unwrap(),
            window_end: EorzeaDuration::new(8, 30, 0).unwrap(),
            bait: Bait::Bait(0),
            previous_weather_set: vec![Weather::Clouds],
            weather_set: vec![Weather::Clouds],
            tug: Tug::Light,
            hookset: Hookset::Precision,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            patch: (7, 0),
            intuition: None,
            lure: Lure::Modest,
            lure_proc: false,
        };
        let result = fish
            .next_window(
                EorzeaTime::new(1, 1, 2, 0, 0, 0).unwrap(),
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                1000,
            )
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 3, 7, 30, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 3, 8, 30, 0).unwrap());
    }

    #[test]
    pub fn next_window_merges_contiguous_weather_periods() {
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new(
                "Region".to_string(),
                WeatherForecast::new("Region".to_string(), vec![(100, Weather::Clouds)]),
            )),
        ));
        let fish = Fish::new(
            0,
            "".to_string(),
            location,
            EorzeaDuration::new(12, 0, 0).unwrap(),
            EorzeaDuration::new(20, 0, 0).unwrap(),
            Bait::Unknown,
            vec![],
            vec![Weather::Clouds],
            Tug::Unknown,
            Hookset::Unknown,
            None,
            Lure::Modest,
            false,
            false,
            false,
            false,
            false,
            false,
            (1, 0),
        );

        let window = fish
            .next_window(
                EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(),
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(window.start(), EorzeaTime::new(1, 1, 1, 12, 0, 0).unwrap());
        assert_eq!(window.end(), EorzeaTime::new(1, 1, 1, 20, 0, 0).unwrap());
    }

    #[test]
    pub fn next_window_day_border() {
        let weather = WeatherForecast::new(
            "Region".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let fishing_hole = FishingHole {
            id: 0,
            name: "Fishing Hole".to_string(),
            region: Rc::new(Region {
                name: "Region".to_string(),
                weather,
            }),
        };
        let fish = Fish {
            id: 0,
            name: "".to_string(),
            location: Rc::new(fishing_hole),
            window_start: EorzeaDuration::new(23, 30, 0).unwrap(),
            window_end: EorzeaDuration::new(1, 0, 0).unwrap(),
            bait: Bait::Bait(0),
            previous_weather_set: vec![Weather::Clouds],
            weather_set: vec![Weather::Clouds],
            tug: Tug::Light,
            hookset: Hookset::Precision,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            patch: (7, 0),
            intuition: None,
            lure: Lure::Modest,
            lure_proc: false,
        };
        let result = fish
            .next_window(
                EorzeaTime::new(1, 1, 3, 0, 0, 0).unwrap(),
                false,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                1_000,
            )
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 4, 23, 30, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 5, 0, 0, 0).unwrap());
    }

    #[test]
    pub fn fish_eyes_ignores_supported_fish_time_window() {
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new(
                "Region".to_string(),
                WeatherForecast::new("Region".to_string(), vec![(100, Weather::Sunny)]),
            )),
        ));
        let make_fish = |fish_eyes| {
            Fish::new(
                0,
                "".to_string(),
                Rc::clone(&location),
                EorzeaDuration::new(1, 0, 0).unwrap(),
                EorzeaDuration::new(2, 0, 0).unwrap(),
                Bait::Unknown,
                vec![],
                vec![],
                Tug::Unknown,
                Hookset::Unknown,
                None,
                Lure::Modest,
                false,
                false,
                false,
                false,
                fish_eyes,
                false,
                (1, 0),
            )
        };
        let start = EorzeaTime::new(1, 1, 2, 2, 0, 0).unwrap();

        let disabled = make_fish(true)
            .next_window(
                start,
                true,
                false,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(disabled.start(), EorzeaTime::new(1, 1, 3, 1, 0, 0).unwrap());
        assert_eq!(disabled.end(), EorzeaTime::new(1, 1, 3, 2, 0, 0).unwrap());

        let unsupported = make_fish(false)
            .next_window(
                start,
                true,
                false,
                true,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(
            unsupported.start(),
            EorzeaTime::new(1, 1, 3, 1, 0, 0).unwrap()
        );
        assert_eq!(
            unsupported.end(),
            EorzeaTime::new(1, 1, 3, 2, 0, 0).unwrap()
        );

        let supported = make_fish(true)
            .next_window(
                start,
                true,
                false,
                true,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert_eq!(
            supported.start(),
            EorzeaTime::new(1, 1, 2, 0, 0, 0).unwrap()
        );
        assert_eq!(supported.end(), EorzeaTime::new(1, 1, 3, 0, 0, 0).unwrap());
    }

    #[test]
    pub fn fish_eyes_preserves_weather_restriction() {
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new(
                "Region".to_string(),
                WeatherForecast::new("Region".to_string(), vec![(100, Weather::Sunny)]),
            )),
        ));
        let fish = Fish::new(
            0,
            "".to_string(),
            location,
            EorzeaDuration::new(1, 0, 0).unwrap(),
            EorzeaDuration::new(2, 0, 0).unwrap(),
            Bait::Unknown,
            vec![],
            vec![Weather::Clouds],
            Tug::Unknown,
            Hookset::Unknown,
            None,
            Lure::Modest,
            false,
            false,
            false,
            false,
            true,
            false,
            (1, 0),
        );

        assert!(
            fish.next_window(
                EorzeaTime::new(1, 1, 2, 2, 0, 0).unwrap(),
                true,
                false,
                true,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                1000,
            )
            .is_none()
        );
    }

    #[test]
    pub fn intuition_window_is_limited_by_intuition_length() {
        let weather = WeatherForecast::new("Region".to_string(), vec![(100, Weather::Sunny)]);
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new("Region".to_string(), weather)),
        ));
        let make_fish = |id, start, end, intuition| {
            Fish::new(
                id,
                "".to_string(),
                Rc::clone(&location),
                EorzeaDuration::new(start, 0, 0).unwrap(),
                EorzeaDuration::new(end, 0, 0).unwrap(),
                Bait::Unknown,
                vec![],
                vec![],
                Tug::Unknown,
                Hookset::Unknown,
                intuition,
                Lure::Modest,
                false,
                false,
                false,
                false,
                false,
                false,
                (1, 0),
            )
        };
        let prerequisite = make_fish(2, 1, 2, None);
        let target = make_fish(
            1,
            3,
            6,
            Some(Intuition::new(Duration::from_secs(350), vec![(1, 2)])),
        );
        let data = FishData::new(
            vec![prerequisite, target],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );
        let target = data.fish_by_id(1).unwrap();
        let window = target
            .next_window(
                EorzeaTime::from_esecs(0),
                false,
                true,
                false,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();

        assert_eq!(window.start(), EorzeaTime::new(1, 1, 1, 3, 0, 0).unwrap());
        assert_eq!(window.end(), EorzeaTime::new(1, 1, 1, 4, 0, 0).unwrap());
        assert!(
            target
                .next_window(EorzeaTime::from_esecs(0), false, true, false, 1, 100)
                .is_none()
        );
    }
}
