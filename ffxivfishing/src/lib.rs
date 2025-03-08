use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
struct FishingHole {
    name: String,
    region: String,
}

#[derive(PartialEq, Eq)]
pub enum Weather {
    Unknown,
    Sunny,
}

pub enum Tug {
    Light,
    Medium,
    Heavy,
}

pub enum Hookset {
    Precision,
    Powerful,
}

pub struct Fish<'a> {
    name: String,
    location: &'a FishingHole,
    start_hour: u8,
    end_hour: u8,
    previous_weather_set: Vec<Weather>,
    weather_set: Vec<Weather>,
    best_catch_path: Vec<Fish<'a>>,
    tug: Tug,
    hookset: Hookset,
    snagging: bool,
    gig: bool,
    folklore: bool,
    fish_eyes: bool,
    patch: (u8, u8),
}

pub struct WeatherForecast {
    region: String,
    weather_rates: Vec<(u8, Weather)>,
}

const EORZEA_WEATHER_PERIOD_IN_SEC: u64 = 1440;

impl WeatherForecast {
    pub fn weather_at(&self, time: SystemTime) -> &Weather {
        let weather_score = time_to_eorzea_weather_score(time);
        self.weather_rates
            .iter()
            .filter(|(n, _)| *n < weather_score)
            .map(|(_, w)| w)
            .next()
            .unwrap_or(&Weather::Unknown)
    }
    pub fn find_pattern(
        &self,
        start: SystemTime,
        previous_weather_set: &[Weather],
        current_weather_set: &[Weather],
        limit: u32,
    ) -> Option<SystemTime> {
        let offset =
            start.duration_since(UNIX_EPOCH).unwrap().as_secs() % EORZEA_WEATHER_PERIOD_IN_SEC;
        let mut time = start - Duration::from_secs(EORZEA_WEATHER_PERIOD_IN_SEC + offset);

        let mut prev_weather = self.weather_at(time);
        for _ in 0..limit {
            time += Duration::from_secs(EORZEA_WEATHER_PERIOD_IN_SEC);
            let current_weather = self.weather_at(time);

            if previous_weather_set.contains(prev_weather)
                && current_weather_set.contains(current_weather)
            {
                return Some(time);
            }
            prev_weather = current_weather;
        }

        None
    }

    pub fn find_next_n_patterns(
        &self,
        n: u8,
        start: SystemTime,
        previous_weather_set: &[Weather],
        current_weather_set: &[Weather],
        limit: u32,
    ) -> Vec<SystemTime> {
        let mut result = Vec::new();
        let mut time = start;
        for _ in 0..n {
            if let Some(t) =
                self.find_pattern(time, previous_weather_set, current_weather_set, limit)
            {
                result.push(t);
            } else {
                break;
            }
            time += Duration::from_secs(EORZEA_WEATHER_PERIOD_IN_SEC);
        }
        result
    }
}

fn time_to_eorzea_weather_score(time: SystemTime) -> u8 {
    let unix_time_sec = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let bell = unix_time_sec / 175;
    let inc = (bell + 8 - (bell % 8)) % 24;
    let total_days = unix_time_sec / 4200;
    let calc_base: u32 = ((total_days * 100) + inc) as u32;
    let step_1: u32 = (calc_base << 11) ^ calc_base;
    let step_2: u32 = (step_1 >> 8) ^ step_1;
    (step_2 % 100) as u8
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn eorzea_time_conversion() {
        let result = time_to_eorzea_weather_score(SystemTime::UNIX_EPOCH);
        assert_eq!(result, 56);
        let result2 =
            time_to_eorzea_weather_score(SystemTime::UNIX_EPOCH + Duration::from_secs(100_000));
        assert_eq!(result2, 76);
        let result3 = time_to_eorzea_weather_score(
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_741_463_853),
        );
        assert_eq!(result3, 94);
    }
}
