use std::collections::HashMap;

use crate::{
    eorzea_time::EorzeaTime,
    fish::{
        DEFAULT_INTUITION_LOOKBACK_MINUTES, DEFAULT_WINDOW_SEARCH_LIMIT, Fish, FishData, FishWindow,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UptimeOp {
    Greater,
    Less,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Name(String),
    Zone(String),
    Patch(String),
    Uptime { op: UptimeOp, threshold: f64 },
    Within(u64),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    clauses: Vec<Clause>,
}

impl Query {
    pub fn clauses(&self) -> &[Clause] {
        &self.clauses
    }
}

pub fn parse_query(raw: &str) -> Query {
    let clauses = raw.split_whitespace().filter_map(parse_clause).collect();
    Query { clauses }
}

fn parse_clause(part: &str) -> Option<Clause> {
    let lower = part.to_lowercase();
    if let Some(rest) = lower.strip_prefix("zone:") {
        return Some(Clause::Zone(rest.trim().to_string()));
    }
    if let Some(rest) = lower.strip_prefix("patch:") {
        return Some(Clause::Patch(rest.trim().to_string()));
    }
    if lower.starts_with("uptime>") || lower.starts_with("uptime<") {
        let op = if lower.starts_with("uptime>") {
            UptimeOp::Greater
        } else {
            UptimeOp::Less
        };
        let raw = &part[7..];
        let (num_str, is_pct) = match raw.strip_suffix('%') {
            Some(s) => (s.trim(), true),
            None => (raw.trim(), false),
        };
        let value: f64 = num_str.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        let threshold = if is_pct { value / 100.0 } else { value };
        return Some(Clause::Uptime { op, threshold });
    }
    if let Some(_rest) = lower.strip_prefix("within:") {
        return parse_duration(&part[7..]).map(Clause::Within);
    }
    Some(Clause::Name(lower.trim().to_string()))
}

fn parse_duration(raw: &str) -> Option<u64> {
    let s = raw.trim();
    let end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num_str, rest) = s.split_at(end);
    let value: f64 = num_str.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let multiplier = match rest.trim().to_lowercase().as_str() {
        "" | "m" => 60.0,
        "s" => 1.0,
        "h" => 3600.0,
        "d" => 86400.0,
        _ => return None,
    };
    let secs = value * multiplier;
    if !secs.is_finite() {
        return None;
    }
    Some(secs as u64)
}

fn matches_clause(
    clause: &Clause,
    fish: &Fish,
    now: EorzeaTime,
    use_fish_eyes: bool,
    cache: &mut NextWindowCache,
    fish_data: &FishData,
) -> bool {
    match clause {
        Clause::Name(query) => fish.name.to_lowercase().contains(query),
        Clause::Zone(query) => {
            fish.location.region().name().to_lowercase().contains(query)
                || fish.location.name().to_lowercase().contains(query)
        }
        Clause::Patch(query) => fish.patch_string().contains(query),
        Clause::Uptime { op, threshold } => {
            let uptime = fish.uptime();
            match op {
                UptimeOp::Greater => uptime > *threshold,
                UptimeOp::Less => uptime < *threshold,
            }
        }
        Clause::Within(secs) => {
            let horizon = now.unix_secs().saturating_add(*secs);
            let now_secs = now.unix_secs();
            match cache.next_window(fish_data, fish.id, now, use_fish_eyes) {
                Some(window) => {
                    window.start().unix_secs() <= horizon && window.end().unix_secs() > now_secs
                }
                None => false,
            }
        }
    }
}

pub fn filter_fish(
    fish_data: &FishData,
    cache: &mut NextWindowCache,
    query: &str,
    now: EorzeaTime,
    use_fish_eyes: bool,
) -> Vec<u32> {
    let query = parse_query(query);
    fish_data
        .fishes()
        .iter()
        .filter(|fish| {
            query
                .clauses
                .iter()
                .all(|clause| matches_clause(clause, fish, now, use_fish_eyes, cache, fish_data))
        })
        .map(|fish| fish.id)
        .collect()
}

#[derive(Default)]
pub struct NextWindowCache {
    bucket: u64,
    entries: HashMap<(u32, bool), Option<FishWindow>>,
}

impl NextWindowCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn bucket_for(now: EorzeaTime) -> u64 {
        now.unix_secs() / 60
    }

    pub fn next_window(
        &mut self,
        fish_data: &FishData,
        fish_id: u32,
        now: EorzeaTime,
        use_fish_eyes: bool,
    ) -> Option<FishWindow> {
        let bucket = Self::bucket_for(now);
        if bucket != self.bucket {
            self.bucket = bucket;
            self.entries.clear();
        }
        let key = (fish_id, use_fish_eyes);
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }
        let window = fish_data.fish_by_id(fish_id).and_then(|fish| {
            fish.next_window(
                now,
                true,
                true,
                use_fish_eyes,
                DEFAULT_INTUITION_LOOKBACK_MINUTES,
                DEFAULT_WINDOW_SEARCH_LIMIT,
            )
        });
        self.entries.insert(key, window.clone());
        window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_name_and_combination() {
        let q = parse_query("Glowing");
        assert_eq!(q.clauses, vec![Clause::Name("glowing".to_string())]);

        let q = parse_query("zone:Mist uptime>1%");
        assert_eq!(
            q.clauses,
            vec![
                Clause::Zone("mist".to_string()),
                Clause::Uptime {
                    op: UptimeOp::Greater,
                    threshold: 0.01
                },
            ]
        );
    }

    #[test]
    fn parse_query_patch_and_uptime() {
        let q = parse_query("patch:6.0");
        assert_eq!(q.clauses, vec![Clause::Patch("6.0".to_string())]);

        let q = parse_query("uptime<0.1%");
        assert_eq!(
            q.clauses,
            vec![Clause::Uptime {
                op: UptimeOp::Less,
                threshold: 0.001
            }]
        );

        let q = parse_query("uptime>5");
        assert_eq!(
            q.clauses,
            vec![Clause::Uptime {
                op: UptimeOp::Greater,
                threshold: 5.0
            }]
        );
    }

    #[test]
    fn parse_query_within_durations() {
        assert_eq!(
            parse_query("within:30m").clauses,
            vec![Clause::Within(1800)]
        );
        assert_eq!(parse_query("within:30").clauses, vec![Clause::Within(1800)]);
        assert_eq!(parse_query("within:45s").clauses, vec![Clause::Within(45)]);
        assert_eq!(parse_query("within:2h").clauses, vec![Clause::Within(7200)]);
        assert_eq!(
            parse_query("within:1d").clauses,
            vec![Clause::Within(86400)]
        );
    }

    #[test]
    fn parse_query_invalid_clauses_are_ignored() {
        assert_eq!(parse_query("uptime>abc").clauses, vec![]);
        assert_eq!(parse_query("within:xx").clauses, vec![]);
        assert_eq!(parse_query("within:30min").clauses, vec![]);
        assert_eq!(
            parse_query("zone:Mist uptime>x name").clauses,
            vec![
                Clause::Zone("mist".to_string()),
                Clause::Name("name".to_string())
            ]
        );
    }

    #[test]
    fn patch_string_formats_tens_as_single_digit() {
        let data = crate::carbuncledata::carbuncle_fishes().unwrap();
        // Fish from patch 7.0 (patch tuple (7, 0)) should render as "7.0".
        let fish = data.fishes().iter().find(|f| f.patch == (7, 0)).unwrap();
        assert_eq!(fish.patch_string(), "7.0");
    }

    #[test]
    fn filter_by_name_and_zone() {
        let data = crate::carbuncledata::carbuncle_fishes().unwrap();
        let mut cache = NextWindowCache::new();
        let now = EorzeaTime::from_esecs(0);

        let ids = filter_fish(&data, &mut cache, "warden of the seven hues", now, false);
        assert!(ids.contains(&24994));

        let ids = filter_fish(&data, &mut cache, "zone:mist", now, false);
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| {
            let fish = data.fish_by_id(*id).unwrap();
            fish.location
                .region()
                .name()
                .to_lowercase()
                .contains("mist")
                || fish.location.name().to_lowercase().contains("mist")
        }));
    }
}
