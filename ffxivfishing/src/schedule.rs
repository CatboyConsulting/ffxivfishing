use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    limit: u32,
    timezone_offset_secs: i32,
    filter_intuition: bool,
    use_fish_eyes: bool,
    include_ongoing: bool,
) -> Vec<FishWindow> {
    let now = timestamp.to_system_time();
    let end = now + Duration::from_secs(timeperiod_secs);
    let mut windows = Vec::new();
    let mut current = timestamp;
    let mut include_current_ongoing = include_ongoing;

    while windows.len() < limit as usize {
        let window = match fish.next_window_with_fish_eyes(
            current,
            include_current_ongoing,
            filter_intuition,
            use_fish_eyes,
            DEFAULT_INTUITION_LOOKBACK_MINUTES,
            DEFAULT_WINDOW_SEARCH_LIMIT,
        ) {
            Some(window) => window,
            None => break,
        };
        let window_start = window.start().to_system_time();
        let window_end = window.end().to_system_time();

        if window_start > end {
            break;
        }

        let next_current = window.end();
        if window_end > now
            && window_overlaps_any_schedule(
                window_start,
                window_end,
                schedule,
                timezone_offset_secs,
            )
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
    timezone_offset_secs: i32,
) -> bool {
    let start_secs = window_start
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let end_secs = window_end
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let offset = timezone_offset_secs as i64;
    let local_start = start_secs + offset;
    let local_end = end_secs + offset;
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

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bst_offset() -> i32 {
        3_600
    }

    fn gmt_offset() -> i32 {
        0
    }

    fn bst_secs() -> u64 {
        1_721_001_600
    }

    fn gmt_secs() -> u64 {
        1_705_276_800
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
            bst_offset(),
        ));
        assert!(window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + 78_960),
            UNIX_EPOCH + Duration::from_secs(b + 79_350),
            &schedule,
            bst_offset(),
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
            bst_offset(),
        ));
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + window_start),
            UNIX_EPOCH + Duration::from_secs(gmt_secs() + window_end),
            &schedule,
            gmt_offset(),
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
            gmt_offset(),
        ));
        assert!(!window_overlaps_any_schedule(
            UNIX_EPOCH + Duration::from_secs(b + DAY_SECS as u64 + 82_860),
            UNIX_EPOCH + Duration::from_secs(b + DAY_SECS as u64 + 82_920),
            &schedule,
            gmt_offset(),
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
            gmt_offset(),
        ));
    }
}
