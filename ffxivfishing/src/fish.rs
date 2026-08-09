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
    fish_eyes: bool,
    collectable: bool,
    previous_weather_set: Rc<[Weather]>,
    weather_set: Rc<[Weather]>,
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

    fn last_windows_in(
        &self,
        start: EorzeaTime,
        end: EorzeaTime,
        use_fish_eyes: bool,
    ) -> PrerequisiteWindows {
        if start >= end {
            return PrerequisiteWindows { with_fish_eyes: None, without_fish_eyes: None };
        }

        let mut last_without = None;
        let mut last_with = None;
        let mut time = start;
        let mut limit = INTUITION_SEARCH_LIMIT;

        if self.previous_weather_set.is_empty() && self.weather_set.is_empty() {
            while limit > 0 {
                let window = self.window_on_day(time);
                if window.start() >= end {
                    break;
                }
                if window.end() > start {
                    last_without = Some(window);
                }
                if use_fish_eyes && self.fish_eyes {
                    let mut fe_day = time;
                    fe_day.round(EORZEA_SUN);
                    let fe_span = EorzeaTimeSpan::new(fe_day, EORZEA_SUN);
                    if fe_span.start() < end && fe_span.end() > start {
                        last_with = Some(fe_span);
                    }
                }
                time += EORZEA_SUN;
                limit -= 1;
            }
        } else {
            while limit > 0 {
                let next_weather = self.location.region.weather.find_pattern(
                    time,
                    &self.previous_weather_set,
                    &self.weather_set,
                    limit,
                );
                let next_weather = match next_weather {
                    Some(nw) => nw,
                    None => break,
                };
                if next_weather >= end {
                    break;
                }
                let weather_span = EorzeaTimeSpan::new(next_weather, EORZEA_WEATHER_PERIOD);

                if let Ok(window) = self.window_on_day(time).overlap(&weather_span) {
                    if window.start() < end && window.end() > start {
                        last_without = merge_window(last_without, window);
                    }
                }

                if use_fish_eyes && self.fish_eyes {
                    let mut fe_day = time;
                    fe_day.round(EORZEA_SUN);
                    let fe_span = EorzeaTimeSpan::new(fe_day, EORZEA_SUN);
                    if let Ok(window) = fe_span.overlap(&weather_span) {
                        if window.start() < end && window.end() > start {
                            last_with = merge_window(last_with, window);
                        }
                    }
                }

                time = next_weather;
                limit -= 1;
            }
        }

        let with_fish_eyes = if use_fish_eyes { last_with } else { last_without };
        PrerequisiteWindows { with_fish_eyes, without_fish_eyes: last_without }
    }
}

fn merge_window(last: Option<EorzeaTimeSpan>, current: EorzeaTimeSpan) -> Option<EorzeaTimeSpan> {
    match last {
        Some(prev) if current.start() == prev.end() => {
            EorzeaTimeSpan::new_start_end(prev.start(), current.end()).ok()
        }
        _ => Some(current),
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
    pub previous_weather_set: Rc<[Weather]>,
    pub weather_set: Rc<[Weather]>,
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
    pub collectable: bool,
    pub patch: (u8, u8),
}

#[derive(Debug, Clone)]
pub struct FishWindow {
    span: EorzeaTimeSpan,
    fish_eyes: bool,
    intuition: Option<IntuitionWindow>,
}

impl FishWindow {
    fn new(span: EorzeaTimeSpan, fish_eyes: bool, intuition: Option<IntuitionWindow>) -> Self {
        Self {
            span,
            fish_eyes,
            intuition,
        }
    }

    pub fn start(&self) -> EorzeaTime {
        self.span.start()
    }

    pub fn end(&self) -> EorzeaTime {
        self.span.end()
    }

    pub fn duration(&self) -> EorzeaDuration {
        self.span.duration()
    }

    pub fn uses_fish_eyes(&self) -> bool {
        self.fish_eyes
    }

    pub fn intuition(&self) -> Option<&IntuitionWindow> {
        self.intuition.as_ref()
    }

    pub fn as_time_span(&self) -> &EorzeaTimeSpan {
        &self.span
    }
}

#[derive(Debug, Clone)]
pub struct IntuitionWindow {
    prerequisite_windows: Vec<IntuitionWindowSetup>,
}

impl IntuitionWindow {
    pub fn prerequisite_windows(&self) -> &[IntuitionWindowSetup] {
        &self.prerequisite_windows
    }
}

#[derive(Debug, Clone)]
pub struct IntuitionWindowSetup {
    amount: u8,
    fish: u32,
    window: EorzeaTimeSpan,
    fish_eyes: bool,
}

impl IntuitionWindowSetup {
    pub fn amount(&self) -> u8 {
        self.amount
    }

    pub fn fish(&self) -> u32 {
        self.fish
    }

    pub fn window(&self) -> &EorzeaTimeSpan {
        &self.window
    }

    pub fn uses_fish_eyes(&self) -> bool {
        self.fish_eyes
    }
}

struct IntuitionPrerequisiteCalculation {
    amount: u8,
    fish: u32,
    collectable: bool,
    window: EorzeaTimeSpan,
    without_fish_eyes: Option<EorzeaTimeSpan>,
}

struct PrerequisiteWindows {
    with_fish_eyes: Option<EorzeaTimeSpan>,
    without_fish_eyes: Option<EorzeaTimeSpan>,
}

fn intuition_window_for_prerequisites(
    window: &EorzeaTimeSpan,
    intuition_length: EorzeaDuration,
    prerequisites: &[IntuitionPrerequisiteCalculation],
    fish_eyes_disabled_for: Option<usize>,
) -> Option<EorzeaTimeSpan> {
    let mut last_prerequisite: Option<&EorzeaTimeSpan> = None;
    for (index, prerequisite) in prerequisites.iter().enumerate() {
        let prerequisite_window = if fish_eyes_disabled_for == Some(index) {
            prerequisite.without_fish_eyes.as_ref()?
        } else {
            &prerequisite.window
        };
        if prerequisite.collectable {
            continue;
        }
        if last_prerequisite.is_none_or(|last| prerequisite_window.end() > last.end()) {
            last_prerequisite = Some(prerequisite_window);
        }
    }

    let last_prerequisite = match last_prerequisite {
        Some(last_prerequisite) => last_prerequisite,
        None => return Some(*window),
    };
    intuition_window_from_last_prerequisite(window, intuition_length, Some(last_prerequisite))
}

fn intuition_window_from_last_prerequisite(
    window: &EorzeaTimeSpan,
    intuition_length: EorzeaDuration,
    last_prerequisite: Option<&EorzeaTimeSpan>,
) -> Option<EorzeaTimeSpan> {
    let Some(last_prerequisite) = last_prerequisite else {
        return Some(window.clone());
    };
    let start = std::cmp::max(window.start(), last_prerequisite.start());
    let end = std::cmp::min(window.end(), last_prerequisite.end() + intuition_length);
    EorzeaTimeSpan::new_start_end(start, end).ok()
}

fn fish_eyes_required_for_prerequisites(
    window: &EorzeaTimeSpan,
    intuition_length: EorzeaDuration,
    prerequisites: &[IntuitionPrerequisiteCalculation],
    intuition_window: &EorzeaTimeSpan,
    use_fish_eyes: bool,
) -> Vec<bool> {
    if !use_fish_eyes {
        return vec![false; prerequisites.len()];
    }

    let mut latest = None;
    let mut second_latest = None;
    for (index, prerequisite) in prerequisites.iter().enumerate() {
        if prerequisite.collectable {
            continue;
        }
        if latest.is_none_or(|(_, last): (usize, &EorzeaTimeSpan)| {
            prerequisite.window.end() > last.end()
        }) {
            second_latest = latest;
            latest = Some((index, &prerequisite.window));
        } else if second_latest.is_none_or(|(_, last): (usize, &EorzeaTimeSpan)| {
            prerequisite.window.end() > last.end()
        }) {
            second_latest = Some((index, &prerequisite.window));
        }
    }

    prerequisites
        .iter()
        .enumerate()
        .map(|(index, prerequisite)| {
            let Some(without_fish_eyes) = prerequisite.without_fish_eyes.as_ref() else {
                return true;
            };
            if prerequisite.collectable {
                return false;
            }

            let last_without_this = if latest.is_some_and(|(last_index, _)| last_index == index) {
                second_latest
            } else {
                latest
            };
            let replacement = match last_without_this {
                Some((_, last)) if last.end() > without_fish_eyes.end() => Some(last),
                Some((last_index, last)) if last.end() == without_fish_eyes.end() => {
                    if last_index < index {
                        Some(last)
                    } else {
                        Some(without_fish_eyes)
                    }
                }
                _ => Some(without_fish_eyes),
            };
            intuition_window_from_last_prerequisite(window, intuition_length, replacement)
                != Some(*intuition_window)
        })
        .collect()
}

pub const DEFAULT_INTUITION_LOOKBACK_MINUTES: u64 = 30;
pub const DEFAULT_WINDOW_SEARCH_LIMIT: u32 = 10_000;
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
            previous_weather_set: Rc::from(previous_weather_set),
            weather_set: Rc::from(weather_set),
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
            collectable: false,
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
        limit: u32,
    ) -> Option<EorzeaTimeSpan> {
        self.next_window_with_fish_eyes(
            start,
            include_ongoing,
            filter_intuition,
            use_fish_eyes,
            intuition_lookback_minutes,
            limit,
        )
        .map(|window| window.span)
    }

    pub fn next_window_with_fish_eyes(
        &self,
        start: EorzeaTime,
        include_ongoing: bool,
        filter_intuition: bool,
        use_fish_eyes: bool,
        intuition_lookback_minutes: u64,
        mut limit: u32,
    ) -> Option<FishWindow> {
        let definition = self.window_definition(use_fish_eyes);
        let mut time = start;
        while limit > 0 {
            let include_target_ongoing = include_ongoing
                || (filter_intuition
                    && self.intuition.is_some()
                    && (self.is_always_available() || (use_fish_eyes && self.fish_eyes)));
            let window = definition.next_window(time, include_target_ongoing, limit)?;
            if !filter_intuition {
                let uses_fish_eyes = use_fish_eyes
                    && self.fish_eyes
                    && !self.is_always_available()
                    && !self.natural_window_contains(&window);
                return Some(FishWindow::new(window, uses_fish_eyes, None));
            }
            match self.intuition_window(&window, intuition_lookback_minutes, use_fish_eyes) {
                Some((window, uses_fish_eyes, intuition)) if window.end() > time => {
                    return Some(FishWindow::new(window, uses_fish_eyes, intuition));
                }
                _ => (),
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

    pub fn next_windows(
        &self,
        start: EorzeaTime,
        limit: u32,
        filter_intuition: bool,
        use_fish_eyes: bool,
        include_ongoing: bool,
        intuition_lookback_minutes: u64,
    ) -> Vec<EorzeaTimeSpan> {
        self.next_windows_with_fish_eyes(
            start,
            limit,
            filter_intuition,
            use_fish_eyes,
            include_ongoing,
            intuition_lookback_minutes,
        )
        .into_iter()
        .map(|window| window.span)
        .collect()
    }

    pub fn next_windows_with_fish_eyes(
        &self,
        start: EorzeaTime,
        limit: u32,
        filter_intuition: bool,
        use_fish_eyes: bool,
        include_ongoing: bool,
        intuition_lookback_minutes: u64,
    ) -> Vec<FishWindow> {
        let mut windows = Vec::new();
        let mut current_time = start;
        let mut remaining = limit;
        let mut include_current_ongoing = include_ongoing;

        while remaining > 0 {
            let window = match self.next_window_with_fish_eyes(
                current_time,
                include_current_ongoing,
                filter_intuition,
                use_fish_eyes,
                intuition_lookback_minutes,
                remaining,
            ) {
                Some(window) => window,
                None => break,
            };
            current_time = window.end();
            include_current_ongoing = false;
            remaining -= 1;
            windows.push(window);
        }

        windows
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
            fish_eyes: self.fish_eyes,
            collectable: self.collectable,
            previous_weather_set: Rc::clone(&self.previous_weather_set),
            weather_set: Rc::clone(&self.weather_set),
        }
    }

    fn intuition_window(
        &self,
        window: &EorzeaTimeSpan,
        intuition_lookback_minutes: u64,
        use_fish_eyes: bool,
    ) -> Option<(EorzeaTimeSpan, bool, Option<IntuitionWindow>)> {
        let (window, prerequisite_windows) =
            self.calculate_intuition_window(window, intuition_lookback_minutes, use_fish_eyes)?;
        let target_requires_fish_eyes = use_fish_eyes
            && self.fish_eyes
            && !self.is_always_available()
            && !self.natural_window_contains(&window);
        Some((
            window,
            target_requires_fish_eyes,
            (!prerequisite_windows.is_empty()).then_some(IntuitionWindow {
                prerequisite_windows,
            }),
        ))
    }

    fn calculate_intuition_window(
        &self,
        window: &EorzeaTimeSpan,
        intuition_lookback_minutes: u64,
        use_fish_eyes: bool,
    ) -> Option<(EorzeaTimeSpan, Vec<IntuitionWindowSetup>)> {
        let intuition = match &self.intuition {
            Some(intuition) => intuition,
            None => return Some((*window, Vec::new())),
        };
        let intuition_length = match intuition.length_eorzea() {
            Some(length) => length,
            None => return Some((*window, Vec::new())),
        };
        let resolved_requirements = if let Some(requirements) = &intuition.resolved_requirements {
            requirements
        } else {
            return None;
        };

        let lookback_esecs = ((intuition_lookback_minutes as u128 * 60 * 3_600 + 87) / 175)
            .min(u64::MAX as u128) as u64;
        let lookback = EorzeaDuration::from_esecs(lookback_esecs);
        let preparation_start = window.start() - lookback;
        let preparation_end = window.end();

        let mut prerequisite_calculations = Vec::new();
        for ((amount, fish), (_, prerequisite)) in
            intuition.requirements.iter().zip(resolved_requirements)
        {
            let prerequisite = prerequisite.as_ref()?;
            let windows =
                prerequisite.last_windows_in(preparation_start, preparation_end, use_fish_eyes);
            let without_fish_eyes = windows.without_fish_eyes;
            let prerequisite_window = if use_fish_eyes {
                windows.with_fish_eyes?
            } else {
                without_fish_eyes?
            };
            prerequisite_calculations.push(IntuitionPrerequisiteCalculation {
                amount: *amount,
                fish: *fish,
                collectable: prerequisite.collectable,
                window: prerequisite_window,
                without_fish_eyes,
            });
        }

        let intuition_window = intuition_window_for_prerequisites(
            window,
            intuition_length,
            &prerequisite_calculations,
            None,
        )?;

        let fish_eyes_required = fish_eyes_required_for_prerequisites(
            window,
            intuition_length,
            &prerequisite_calculations,
            &intuition_window,
            use_fish_eyes,
        );
        let prerequisite_windows = prerequisite_calculations
            .iter()
            .zip(fish_eyes_required)
            .map(|(prerequisite, fish_eyes)| IntuitionWindowSetup {
                amount: prerequisite.amount,
                fish: prerequisite.fish,
                window: if fish_eyes {
                    prerequisite.window
                } else {
                    prerequisite
                        .without_fish_eyes
                        .unwrap_or(prerequisite.window)
                },
                fish_eyes,
            })
            .collect();

        Some((intuition_window, prerequisite_windows))
    }

    fn natural_window_contains(&self, window: &EorzeaTimeSpan) -> bool {
        let definition = self.window_definition(false);
        let mut day = window.start();
        day.round(EORZEA_SUN);
        for offset in [-1i8, 0, 1] {
            let candidate_day = match offset {
                -1 => day - EORZEA_SUN,
                0 => day,
                1 => day + EORZEA_SUN,
                _ => unreachable!(),
            };
            if definition.window_on_day(candidate_day).start() <= window.start()
                && definition.window_on_day(candidate_day).end() >= window.end()
            {
                return true;
            }
        }
        false
    }

    fn is_always_available(&self) -> bool {
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
                let location = Rc::clone(&fish.location);
                intuition.resolved_requirements = Some(
                    intuition
                        .requirements
                        .iter()
                        .map(|(count, id)| {
                            let definition = definitions.get(id).cloned().unwrap_or_else(|| {
                                // Intuition fish missing from the data are treated as always up.
                                FishWindowDefinition {
                                    location: Rc::clone(&location),
                                    window_start: EorzeaDuration::from_esecs(0),
                                    window_end: EorzeaDuration::from_esecs(0),
                                    fish_eyes: false,
                                    collectable: false,
                                    previous_weather_set: Rc::from([]),
                                    weather_set: Rc::from([]),
                                }
                            });
                            (*count, Some(definition))
                        })
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
            previous_weather_set: Rc::from(vec![Weather::Clouds]),
            weather_set: Rc::from(vec![Weather::Clouds]),
            tug: Tug::Light,
            hookset: Hookset::Precision,
            intuition: None,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            collectable: false,
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
            previous_weather_set: Rc::from(vec![Weather::Clouds]),
            weather_set: Rc::from(vec![Weather::Clouds]),
            tug: Tug::Light,
            hookset: Hookset::Precision,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            collectable: false,
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
            previous_weather_set: Rc::from(vec![Weather::Clouds]),
            weather_set: Rc::from(vec![Weather::Clouds]),
            tug: Tug::Light,
            hookset: Hookset::Precision,
            snagging: false,
            gig: false,
            folklore: false,
            fish_eyes: false,
            big_fish: false,
            collectable: false,
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

        let supported_tagged = make_fish(true)
            .next_window_with_fish_eyes(
                start,
                true,
                false,
                true,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                100,
            )
            .unwrap();
        assert!(supported_tagged.uses_fish_eyes());
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
            .next_window_with_fish_eyes(
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
        let setup = window
            .intuition()
            .unwrap()
            .prerequisite_windows()
            .first()
            .unwrap();
        assert_eq!(setup.amount(), 1);
        assert_eq!(setup.fish(), 2);
        assert_eq!(
            setup.window().start(),
            EorzeaTime::new(1, 1, 1, 1, 0, 0).unwrap()
        );
        assert_eq!(
            setup.window().end(),
            EorzeaTime::new(1, 1, 1, 2, 0, 0).unwrap()
        );
        assert!(
            target
                .next_window(EorzeaTime::from_esecs(0), false, true, false, 1, 100)
                .is_none()
        );
    }

    #[test]
    pub fn aquamaton_has_more_windows_with_fish_eyes() {
        let data = crate::carbuncledata::carbuncle_fishes().unwrap();
        let aquamaton = data.fish_by_id(33240).unwrap();

        let without_fish_eyes = aquamaton.next_windows(
            EorzeaTime::from_esecs(0),
            100,
            true,
            false,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );
        let with_fish_eyes = aquamaton.next_windows(
            EorzeaTime::from_esecs(0),
            100,
            true,
            true,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );

        assert!(
            with_fish_eyes.len() > without_fish_eyes.len(),
            "Fish Eyes should add Aquamaton windows: {} with vs {} without",
            with_fish_eyes.len(),
            without_fish_eyes.len()
        );

        let without_fish_eyes_tagged = aquamaton.next_windows_with_fish_eyes(
            EorzeaTime::from_esecs(0),
            100,
            true,
            false,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );
        let with_fish_eyes_tagged = aquamaton.next_windows_with_fish_eyes(
            EorzeaTime::from_esecs(0),
            100,
            true,
            true,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );
        assert!(
            without_fish_eyes_tagged
                .iter()
                .all(|window| !window.uses_fish_eyes())
        );
        assert!(with_fish_eyes_tagged.iter().any(|window| {
            window.intuition().is_some_and(|intuition| {
                intuition
                    .prerequisite_windows()
                    .iter()
                    .any(|setup| setup.uses_fish_eyes())
            })
        }));
    }

    #[test]
    pub fn stethacanthus_uses_collectable_fish_eyes_prerequisite() {
        let data = crate::carbuncledata::carbuncle_fishes().unwrap();
        let stethacanthus = data.fish_by_id(24992).unwrap();

        let without_fish_eyes = stethacanthus.next_windows(
            EorzeaTime::from_esecs(0),
            100,
            true,
            false,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );
        let with_fish_eyes = stethacanthus.next_windows_with_fish_eyes(
            EorzeaTime::from_esecs(0),
            100,
            true,
            true,
            false,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
        );

        assert!(with_fish_eyes.len() > without_fish_eyes.len());
        assert!(with_fish_eyes.iter().any(|window| {
            window.intuition().is_some_and(|intuition| {
                intuition
                    .prerequisite_windows()
                    .iter()
                    .any(|setup| setup.uses_fish_eyes())
            })
        }));
    }

    #[test]
    pub fn missing_intuition_prerequisite_is_always_available() {
        let location = Rc::new(FishingHole::new(
            0,
            "Fishing Hole".to_string(),
            Rc::new(Region::new(
                "Region".to_string(),
                WeatherForecast::new("Region".to_string(), vec![(100, Weather::Sunny)]),
            )),
        ));
        let target = Fish::new(
            1,
            "Target".to_string(),
            location,
            EorzeaDuration::new(3, 0, 0).unwrap(),
            EorzeaDuration::new(6, 0, 0).unwrap(),
            Bait::Unknown,
            vec![],
            vec![],
            Tug::Unknown,
            Hookset::Unknown,
            Some(Intuition::new(Duration::from_secs(350), vec![(1, 2)])),
            Lure::Modest,
            false,
            false,
            false,
            false,
            false,
            false,
            (1, 0),
        );
        let data = FishData::new(vec![target], vec![], vec![], vec![], HashMap::new());
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
        assert_eq!(window.end(), EorzeaTime::new(1, 1, 1, 6, 0, 0).unwrap());
    }
}
