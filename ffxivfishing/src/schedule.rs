use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Offset, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use crate::{
    eorzea_time::EorzeaTime,
    fish::{DEFAULT_INTUITION_LOOKBACK_MINUTES, DEFAULT_WINDOW_SEARCH_LIMIT, Fish, FishWindow},
};

const DAY_SECS: i64 = 86_400;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntry {
    #[serde(default)]
    pub day_of_week: Option<u8>,
    pub start_sec: u64,
    pub end_sec: u64,
}

pub fn fish_windows_in_schedule(
    fish: &Fish,
    timestamp: EorzeaTime,
    schedule: &[ScheduleEntry],
    timeperiod_secs: u64,
    timezone: Tz,
    filter_intuition: bool,
    use_fish_eyes: bool,
    include_ongoing: bool,
) -> Vec<FishWindow> {
    let now = timestamp.to_system_time();
    let end = now + Duration::from_secs(timeperiod_secs);
    let mut windows = Vec::new();
    let mut current = timestamp;
    let mut include_current_ongoing = include_ongoing;

    while let Some(window) = fish.next_window_with_fish_eyes(
        current,
        include_current_ongoing,
        filter_intuition,
        use_fish_eyes,
        DEFAULT_INTUITION_LOOKBACK_MINUTES,
        DEFAULT_WINDOW_SEARCH_LIMIT,
    ) {
        let window_start = window.start().to_system_time();
        let window_end = window.end().to_system_time();

        if window_start > end {
            break;
        }

        let next_current = window.end();
        if window_end > now
            && window_overlaps_any_schedule(window_start, window_end, schedule, timezone)
        {
            windows.push(window);
        }

        current = next_current;
        include_current_ongoing = false;
    }

    windows
}

fn window_overlaps_any_schedule(
    window_start: SystemTime,
    window_end: SystemTime,
    schedule: &[ScheduleEntry],
    timezone: Tz,
) -> bool {
    let start_secs = window_start
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let end_secs = window_end
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let start_day = start_secs.div_euclid(DAY_SECS);
    let end_day = end_secs.div_euclid(DAY_SECS);
    for day in start_day..=end_day {
        let day_midnight = day * DAY_SECS;
        let day_end = day_midnight + DAY_SECS;
        let portion_start = start_secs.max(day_midnight);
        let portion_end = end_secs.min(day_end);
        if portion_start >= portion_end {
            continue;
        }

        let utc_midnight = Utc.timestamp_opt(day_midnight, 0).single().unwrap();
        let offset = timezone
            .offset_from_utc_datetime(&utc_midnight.naive_utc())
            .fix()
            .local_minus_utc() as i64;
        let local_start = portion_start + offset;
        let local_end = portion_end + offset;
        let first_local_day = local_start.div_euclid(DAY_SECS) - 1;
        let last_local_day = (local_end - 1).div_euclid(DAY_SECS);

        for local_day in first_local_day..=last_local_day {
            let day_of_week = (local_day + 4).rem_euclid(7) as u8;
            let local_day_start = local_day * DAY_SECS;
            for entry in schedule {
                if entry
                    .day_of_week
                    .is_some_and(|entry_day| entry_day != day_of_week)
                {
                    continue;
                }

                let schedule_start = local_day_start + entry.start_sec as i64;
                let mut schedule_end = local_day_start + entry.end_sec as i64;
                if entry.end_sec <= entry.start_sec {
                    schedule_end += DAY_SECS;
                }

                if local_start < schedule_end && local_end > schedule_start {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timezone() -> Tz {
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

    fn evening_schedule(day_of_week: Option<u8>) -> Vec<ScheduleEntry> {
        vec![ScheduleEntry {
            day_of_week,
            start_sec: 82_800,
            end_sec: 86_400,
        }]
    }

    #[test]
    fn timezone_offset_is_applied_to_schedule() {
        let schedule = evening_schedule(None);
        let b = bst_secs();
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 71_760),
            UNIX_EPOCH + Duration::from_secs(b + 72_150),
            &schedule,
            timezone(),
        ));
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 78_960),
            UNIX_EPOCH + Duration::from_secs(b + 79_350),
            &schedule,
            timezone(),
        ));
    }

    #[test]
    fn daylight_saving_changes_matching_utc_window() {
        let schedule = evening_schedule(None);
        let window_start = 78_960;
        let window_end = 79_320;
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(bst_secs() + window_start),
            UNIX_EPOCH + Duration::from_secs(bst_secs() + window_end),
            &schedule,
            timezone(),
        ));
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + window_start),
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + window_end),
            &schedule,
            timezone(),
        ));
    }

    #[test]
    fn day_of_week_filtering_uses_local_day() {
        let schedule = evening_schedule(Some(1));
        let b = gmt_secs();
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 82_860),
            UNIX_EPOCH + Duration::from_secs(b + 82_920),
            &schedule,
            timezone(),
        ));
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + DAY_SECS as u64 + 82_860),
            UNIX_EPOCH + Duration::from_secs(b + DAY_SECS as u64 + 82_920),
            &schedule,
            timezone(),
        ));
    }

    #[test]
    fn overnight_schedule_matches_after_midnight() {
        let schedule = vec![ScheduleEntry {
            day_of_week: None,
            start_sec: 23 * 3_600,
            end_sec: 1 * 3_600,
        }];
        let b = gmt_secs();
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 86_400 + 1_800),
            UNIX_EPOCH + Duration::from_secs(b + 86_400 + 3_600),
            &schedule,
            timezone(),
        ));
    }
}
