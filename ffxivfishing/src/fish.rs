use std::{
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
    Mooch(u32),
    Bait(u32),
    Unknown,
}

#[derive(Debug)]
pub struct Intuition {
    length: Duration,
    requirements: Vec<(u8, u32)>,
}
impl Intuition {
    pub(crate) fn new(length: Duration, requirements: Vec<(u8, u32)>) -> Self {
        Self {
            length,
            requirements,
        }
    }
}

#[derive(Debug)]
pub enum Lure {
    Moderate,
    Ambitious,
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
    pub patch: (u8, u8),
}

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
            patch,
        }
    }

    pub fn window_on_day(&self, etime: EorzeaTime) -> EorzeaTimeSpan {
        let mut day = etime;
        day.round(EORZEA_SUN);
        let start = day + self.window_start;
        let mut end = day + self.window_end;
        if end <= start {
            end += EORZEA_SUN;
        }
        EorzeaTimeSpan::new_start_end(start, end).unwrap()
    }

    pub fn next_window(
        &self,
        start: EorzeaTime,
        include_ongoing: bool,
        mut limit: u32,
    ) -> Option<EorzeaTimeSpan> {
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
                if start <= min_window && window.duration().total_seconds() > 0 {
                    return Some(window);
                }
            }
            time += EORZEA_WEATHER_PERIOD;
            limit -= 1;
        }
        None
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
            Bait::Mooch(id) => Some(id),
            Bait::Bait(id) => Some(id),
            Bait::Unknown => None,
        }
    }
}

impl FishingHole {
    pub fn new(name: String, region: Rc<Region>) -> FishingHole {
        FishingHole { name, region }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Region {
    pub fn new(name: String, weather: WeatherForecast) -> Region {
        Region { name, weather }
    }
    pub fn name(&self) -> &str {
        &self.name
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
}

impl FishData {
    pub fn new(
        fishes: Vec<Fish>,
        fishing_holes: Vec<Rc<FishingHole>>,
        regions: Vec<Rc<Region>>,
        items: Vec<FishingItem>,
    ) -> FishData {
        FishData {
            fishes,
            fishing_holes,
            regions,
            items,
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
            patch: (7, 0),
            lure: Lure::Moderate,
            lure_proc: false,
        };
        let result = fish
            .next_window(EorzeaTime::new(1, 1, 2, 2, 0, 0).unwrap(), 1000)
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 3, 1, 0, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 3, 2, 0, 0).unwrap());
    }

    #[test]
    pub fn next_window_weather_border() {
        let weather = WeatherForecast::new(
            "Region".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let fishing_hole = FishingHole {
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
            patch: (7, 0),
            intuition: None,
            lure: Lure::Moderate,
            lure_proc: false,
        };
        let result = fish
            .next_window(EorzeaTime::new(1, 1, 2, 0, 0, 0).unwrap(), 1000)
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 3, 7, 30, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 3, 8, 0, 0).unwrap());
    }

    #[test]
    pub fn next_window_day_border() {
        let weather = WeatherForecast::new(
            "Region".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let fishing_hole = FishingHole {
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
            patch: (7, 0),
            intuition: None,
            lure: Lure::Moderate,
            lure_proc: false,
        };
        let result = fish
            .next_window(EorzeaTime::new(1, 1, 3, 0, 0, 0).unwrap(), 1_000)
            .unwrap();
        assert_eq!(result.start(), EorzeaTime::new(1, 1, 4, 23, 30, 0).unwrap());
        assert_eq!(result.end(), EorzeaTime::new(1, 1, 5, 0, 0, 0).unwrap());
    }
}
