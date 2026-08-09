use crate::eorzea_time::{EORZEA_WEATHER_PERIOD, EorzeaTime};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Weather {
    Unknown,
    Id(u32),
    Sunny,
    Clouds,
    ClearSkies,
    FairSkies,
    Fog,
    Wind,
}

#[derive(Debug, Clone)]
pub struct WeatherForecast {
    region: String,
    weather_rates: Vec<(u8, Weather)>,
    max_score: u8,
}

impl WeatherForecast {
    pub fn new(region: String, mut weather_rates: Vec<(u8, Weather)>) -> WeatherForecast {
        weather_rates.sort_by_key(|(n, _)| *n);
        let max_score = weather_rates
            .last()
            .map(|(n, _)| *n)
            .unwrap_or(1);
        WeatherForecast {
            region,
            weather_rates,
            max_score,
        }
    }
    pub fn weather_at(&self, time: EorzeaTime) -> &Weather {
        let weather_score = eorzea_weather_score(time, self.max_score);
        for (rate, weather) in &self.weather_rates {
            if *rate > weather_score {
                return weather;
            }
        }
        &Weather::Unknown
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    pub fn find_pattern(
        &self,
        start: EorzeaTime,
        previous_weather_set: &[Weather],
        current_weather_set: &[Weather],
        limit: u32,
    ) -> Option<EorzeaTime> {
        let mut time = start;
        time.round(EORZEA_WEATHER_PERIOD);
        time -= EORZEA_WEATHER_PERIOD;

        let mut prev_weather = self.weather_at(time);
        for _ in 0..limit {
            time += EORZEA_WEATHER_PERIOD;
            let current_weather = self.weather_at(time);
            if (previous_weather_set.is_empty() || previous_weather_set.contains(prev_weather))
                && (current_weather_set.is_empty() || current_weather_set.contains(current_weather))
            {
                return Some(time);
            }
            prev_weather = current_weather;
        }

        None
    }

    pub fn pattern_uptime(
        &self,
        previous_weather_set: &[Weather],
        current_weather_set: &[Weather],
    ) -> f64 {
        let prob = |set: &[Weather]| -> f64 {
            if set.is_empty() {
                return 1.0;
            }
            let max = *self.weather_rates.last().map(|(n, _)| n).unwrap_or(&1) as f64;
            let mut prev = 0u8;
            let mut matched = 0u8;
            for (rate, weather) in &self.weather_rates {
                if set.contains(weather) {
                    matched += rate - prev;
                }
                prev = *rate;
            }
            matched as f64 / max
        };
        prob(previous_weather_set) * prob(current_weather_set)
    }

    pub fn weather_uptime(&self, weather_set: &[Weather]) -> f64 {
        self.pattern_uptime(&[], weather_set)
    }

    pub fn find_next_n_patterns(
        &self,
        n: u8,
        start: EorzeaTime,
        previous_weather_set: &[Weather],
        current_weather_set: &[Weather],
        limit: u32,
    ) -> Vec<EorzeaTime> {
        let mut result = Vec::new();
        let mut time = start;
        for _ in 0..n {
            if let Some(t) =
                self.find_pattern(time, previous_weather_set, current_weather_set, limit)
            {
                result.push(t);
                time = t;
            } else {
                break;
            }
            time += EORZEA_WEATHER_PERIOD;
        }
        result
    }
}

fn eorzea_weather_score(time: EorzeaTime, max_score: u8) -> u8 {
    let timestamp = time.as_esecs();
    let unix_time_sec = (timestamp * 175 + 1800) / 3600;
    let bell = unix_time_sec / 175;
    let inc = (bell + 8 - (bell % 8)) % 24;
    let total_days = unix_time_sec / 4200;
    let calc_base: u32 = ((total_days * 100) + inc) as u32;
    let step_1: u32 = (calc_base << 11) ^ calc_base;
    let step_2: u32 = (step_1 >> 8) ^ step_1;
    (step_2 % (max_score as u32)) as u8
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn eorzea_time_conversion() {
        let result = eorzea_weather_score(EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(), 100);
        assert_eq!(result, 56);
        let result2 =
            eorzea_weather_score(EorzeaTime::new(1, 1, 24, 19, 25, 43).unwrap(), 100);
        assert_eq!(result2, 76);

        let result3 =
            eorzea_weather_score(EorzeaTime::new(2, 1, 1, 0, 0, 0).unwrap(), 100);
        assert_eq!(result3, 78);
    }

    #[test]
    fn pattern_search() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let weather_vec = vec![Weather::Sunny];
        let result = forecast.find_pattern(
            EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap(),
            &weather_vec,
            &weather_vec,
            1000,
        );
        assert_eq!(result, Some(EorzeaTime::new(1, 1, 4, 0, 0, 0).unwrap()));

        let weather_vec2 = vec![Weather::Clouds];
        let result2 = forecast.find_pattern(
            EorzeaTime::new(1, 1, 1, 1, 1, 1).unwrap(),
            &weather_vec2,
            &weather_vec2,
            1000,
        );
        assert_eq!(result2, Some(EorzeaTime::new(1, 1, 1, 16, 0, 0).unwrap()));
    }
    #[test]
    fn weather_at_real() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![
                (20, Weather::Clouds),
                (50, Weather::ClearSkies),
                (80, Weather::FairSkies),
                (90, Weather::Fog),
                (100, Weather::Wind),
            ],
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(100_000)),
            &Weather::FairSkies
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(110_000)),
            &Weather::FairSkies
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(120_000)),
            &Weather::ClearSkies
        );
    }

    #[test]
    fn weather_at_empyrium() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![
                (5, Weather::Clouds), // Weathers not acurate, only scores are relevant
                (25, Weather::ClearSkies),
                (65, Weather::FairSkies),
                (80, Weather::Fog),
                (90, Weather::Wind),
            ],
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(100_000)),
            &Weather::ClearSkies
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(110_000)),
            &Weather::ClearSkies
        );
        assert_eq!(
            forecast.weather_at(EorzeaTime::from_esecs(120_000)),
            &Weather::FairSkies
        );
    }

    #[test]
    fn pattern_search_not_found() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let weather_vec = vec![Weather::Unknown];

        let result = forecast.find_pattern(
            EorzeaTime::from_esecs(10_000),
            &weather_vec,
            &weather_vec,
            1000,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn pattern_uptime() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let sunny = vec![Weather::Sunny];
        assert_eq!(forecast.pattern_uptime(&sunny, &sunny), 0.25);
        assert_eq!(forecast.pattern_uptime(&[], &sunny), 0.5);
        assert_eq!(forecast.pattern_uptime(&[], &[]), 1.0);
        assert_eq!(
            forecast.pattern_uptime(&[Weather::Unknown], &[Weather::Unknown]),
            0.0
        );
    }

    #[test]
    fn pattern_search_n() {
        let forecast = WeatherForecast::new(
            "".to_string(),
            vec![(50, Weather::Clouds), (100, Weather::Sunny)],
        );
        let weather_vec = vec![Weather::Sunny];
        let result = forecast.find_next_n_patterns(
            3,
            EorzeaTime::from_esecs(10_000),
            &weather_vec,
            &weather_vec,
            1000,
        );
        assert_eq!(result.len(), 3);
        assert_eq!(
            result,
            [259_200, 576_000, 662_400]
                .iter()
                .map(|sec| EorzeaTime::from_esecs(*sec))
                .collect::<Vec<EorzeaTime>>()
        );
    }
}
