use config_manager::utils::string::{find_closest_match, levenshtein_distance};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_levenshtein_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein_distance");

    let test_cases = vec![
        ("short", "short", "shorx"),
        ("medium", "production-api-server", "production-api-servr"),
        (
            "long",
            "very-long-configuration-name-with-many-parts",
            "very-long-configuration-name-with-meny-parts",
        ),
    ];

    for (name, s1, s2) in test_cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                let dist = levenshtein_distance(black_box(s1), black_box(s2));
                black_box(dist);
            });
        });
    }

    group.finish();
}

fn bench_find_closest_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_closest_match");

    // Small list
    let small_list: Vec<String> = (0..10).map(|i| format!("entity_{:03}", i)).collect();

    // Medium list
    let medium_list: Vec<String> = (0..100).map(|i| format!("entity_{:03}", i)).collect();

    // Large list
    let large_list: Vec<String> = (0..1000).map(|i| format!("entity_{:04}", i)).collect();

    group.bench_function("small_exact", |b| {
        let target = "entity_005";
        let small_list_refs: Vec<&str> = small_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &small_list_refs, 2);
            black_box(result);
        });
    });

    group.bench_function("small_typo", |b| {
        let target = "entity_00X"; // Typo
        let small_list_refs: Vec<&str> = small_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &small_list_refs, 2);
            black_box(result);
        });
    });

    group.bench_function("medium_exact", |b| {
        let target = "entity_050";
        let medium_list_refs: Vec<&str> = medium_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &medium_list_refs, 2);
            black_box(result);
        });
    });

    group.bench_function("medium_typo", |b| {
        let target = "entity_05X"; // Typo
        let medium_list_refs: Vec<&str> = medium_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &medium_list_refs, 2);
            black_box(result);
        });
    });

    group.bench_function("large_exact", |b| {
        let target = "entity_0500";
        let large_list_refs: Vec<&str> = large_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &large_list_refs, 2);
            black_box(result);
        });
    });

    group.bench_function("large_typo", |b| {
        let target = "entity_050X"; // Typo
        let large_list_refs: Vec<&str> = large_list.iter().map(|s| s.as_str()).collect();
        b.iter(|| {
            let result = find_closest_match(black_box(target), &large_list_refs, 2);
            black_box(result);
        });
    });

    group.finish();
}

fn bench_levenshtein_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein_by_length");

    for len in [5, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::from_parameter(len), &len, |b, &len| {
            let s1: String = (0..len).map(|_| 'a').collect();
            let s2: String = (0..len)
                .map(|i| if i % 3 == 0 { 'b' } else { 'a' })
                .collect();

            b.iter(|| {
                let dist = levenshtein_distance(black_box(&s1), black_box(&s2));
                black_box(dist);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_levenshtein_distance,
    bench_find_closest_match,
    bench_levenshtein_matrix
);
criterion_main!(benches);
