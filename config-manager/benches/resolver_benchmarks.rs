use config_manager::resolver::{HashMapResolver, Resolver};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;

fn create_entity_map(size: usize) -> HashMap<String, String> {
    (0..size)
        .map(|i| {
            let key = format!("entity_{:04}", i);
            let value = format!("value_{}", i);
            (key, value)
        })
        .collect()
}

fn create_complex_names(size: usize) -> HashMap<String, String> {
    let prefixes = ["prod", "staging", "dev", "test"];
    let suffixes = ["api", "web", "db", "cache", "worker"];

    (0..size)
        .map(|i| {
            let prefix = prefixes[i % prefixes.len()];
            let suffix = suffixes[(i / prefixes.len()) % suffixes.len()];
            let num = i / (prefixes.len() * suffixes.len());
            let key = format!("{}-{}-{:02}", prefix, suffix, num);
            let value = format!("value_{}", i);
            (key, value)
        })
        .collect()
}

fn bench_resolver_exact_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolver_exact_match");

    for size in [100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("simple", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(false); // No fuzzy matching for exact
            let target = format!("entity_{:04}", size / 2);

            b.iter(|| {
                let result = resolver.resolve(&map, &target).unwrap();
                black_box(result);
            });
        });

        group.bench_with_input(BenchmarkId::new("complex", size), &size, |b, &size| {
            let map = create_complex_names(size);
            let resolver = HashMapResolver::new(false);
            let keys: Vec<_> = resolver.list_ids(&map);
            let target = &keys[size / 2];

            b.iter(|| {
                let result = resolver.resolve(&map, target).unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_resolver_fuzzy_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolver_fuzzy_match");

    for size in [100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("typo_1_char", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(true).with_max_distance(2);
            // Typo: "entity_0050" -> "entity_005X"
            let target = format!("entity_{:03}X", size / 20);

            b.iter(|| {
                let suggestion = resolver.suggest(&map, &target);
                black_box(suggestion);
            });
        });

        group.bench_with_input(BenchmarkId::new("typo_2_chars", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(true).with_max_distance(2);
            // Typo: "entity_0050" -> "entity_00XX"
            let target = format!("entity_{:02}XX", size / 20);

            b.iter(|| {
                let suggestion = resolver.suggest(&map, &target);
                black_box(suggestion);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("complex_typo", size),
            &size,
            |b, &_size| {
                let map = create_complex_names(size);
                let resolver = HashMapResolver::new(true).with_max_distance(2);
                // Typo: "prod-api-05" -> "prad-api-05"
                let target = "prad-api-05";

                b.iter(|| {
                    let suggestion = resolver.suggest(&map, target);
                    black_box(suggestion);
                });
            },
        );
    }

    group.finish();
}

fn bench_resolver_list_ids(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolver_list_ids");

    for size in [100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("list", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(false);

            b.iter(|| {
                let ids = resolver.list_ids(&map);
                black_box(ids);
            });
        });
    }

    group.finish();
}

fn bench_resolver_exists(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolver_exists");

    for size in [100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("exists", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(false);
            let target = format!("entity_{:04}", size / 2);

            b.iter(|| {
                let exists = resolver.exists(&map, &target);
                black_box(exists);
            });
        });

        group.bench_with_input(BenchmarkId::new("not_exists", size), &size, |b, &size| {
            let map = create_entity_map(size);
            let resolver = HashMapResolver::new(false);
            let target = "nonexistent_entity";

            b.iter(|| {
                let exists = resolver.exists(&map, target);
                black_box(exists);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_resolver_exact_match,
    bench_resolver_fuzzy_match,
    bench_resolver_list_ids,
    bench_resolver_exists
);
criterion_main!(benches);
