use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use ffxivfishing::eorzea_time::{EORZEA_ZERO_TIME, EorzeaTime};
use ffxivfishing::fish::{DEFAULT_INTUITION_LOOKBACK_MINUTES, DEFAULT_WINDOW_SEARCH_LIMIT};

fn fish_cases() -> Vec<(u32, &'static str, EorzeaTime)> {
    vec![
        (24994, "warden_of_the_seven_hues", EORZEA_ZERO_TIME),
        (
            33241,
            "cinder_surprise",
            EorzeaTime::new(1108, 8, 15, 0, 0, 0).unwrap(),
        ),
        (33240, "aquamaton", EORZEA_ZERO_TIME),
    ]
}

fn bench_next_window(c: &mut Criterion) {
    let data = ffxivfishing::carbuncledata::carbuncle_fishes().expect("data parse");

    let mut group = c.benchmark_group("next_window");
    for (id, label, start) in fish_cases() {
        let fish = data.fish_by_id(id).expect("fish id");
        for filter_intuition in [false, true] {
            for fish_eyes in [false, true] {
                let name = format!(
                    "{label}/filter_intuition={filter_intuition}/use_fish_eyes={fish_eyes}"
                );
                group.bench_function(BenchmarkId::new("next_window", &name), |b| {
                    b.iter(|| {
                        fish.next_window(
                            start,
                            true,
                            filter_intuition,
                            fish_eyes,
                            DEFAULT_INTUITION_LOOKBACK_MINUTES,
                            DEFAULT_WINDOW_SEARCH_LIMIT,
                        )
                    });
                });
            }
        }
    }
    group.finish();
}

fn bench_next_windows(c: &mut Criterion) {
    let data = ffxivfishing::carbuncledata::carbuncle_fishes().expect("data parse");

let mut group = c.benchmark_group("next_windows");
    for (id, label, start) in fish_cases() {
        let fish = data.fish_by_id(id).expect("fish id");
        group.bench_function(BenchmarkId::new("limit=20", label), |b| {
            b.iter(|| {
                fish.next_windows(
                    start,
                    20,
                    true,
                    false,
                    true,
                    DEFAULT_INTUITION_LOOKBACK_MINUTES,
                )
            });
        });
    }
    group.finish();
}

fn bench_filter(c: &mut Criterion) {
    use ffxivfishing::filter::{NextWindowCache, filter_fish};

    let data = ffxivfishing::carbuncledata::carbuncle_fishes().expect("data parse");

    let mut group = c.benchmark_group("filter");
    group.bench_function("name", |b| {
        let mut cache = NextWindowCache::new();
        b.iter(|| filter_fish(&data, &mut cache, "warden", EORZEA_ZERO_TIME, false));
    });
    group.bench_function("within_30m_cold", |b| {
        b.iter(|| {
            let mut cache = NextWindowCache::new();
            filter_fish(&data, &mut cache, "within:30m", EORZEA_ZERO_TIME, false)
        });
    });
    group.bench_function("within_30m_warm", |b| {
        let mut cache = NextWindowCache::new();
        filter_fish(&data, &mut cache, "within:30m", EORZEA_ZERO_TIME, false);
        b.iter(|| filter_fish(&data, &mut cache, "within:30m", EORZEA_ZERO_TIME, false));
    });
    group.finish();
}

criterion_group!(benches, bench_next_window, bench_next_windows, bench_filter);
criterion_main!(benches);
