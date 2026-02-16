use config_manager::loader::{ConfigLoader, JsonLoader, LayeredLoader, MergeStrategy};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::{Deserialize, Serialize};
use std::fs;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SmallConfig {
    name: String,
    port: u16,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LargeConfig {
    id: String,
    values: Vec<i32>,
    nested: NestedConfig,
    items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NestedConfig {
    field1: String,
    field2: i32,
    field3: bool,
    deep: DeepNested,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeepNested {
    value: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    name: String,
    value: i32,
}

fn create_small_config() -> SmallConfig {
    SmallConfig {
        name: "test".to_string(),
        port: 8080,
        enabled: true,
    }
}

fn create_large_config() -> LargeConfig {
    LargeConfig {
        id: "large-config-123".to_string(),
        values: (0..1000).collect(),
        nested: NestedConfig {
            field1: "nested value".to_string(),
            field2: 42,
            field3: true,
            deep: DeepNested {
                value: "deep nested value".to_string(),
                count: 100,
            },
        },
        items: (0..100)
            .map(|i| Item {
                name: format!("item_{}", i),
                value: i * 10,
            })
            .collect(),
    }
}

fn bench_json_load(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();

    let small_path = temp_dir.path().join("small.json");
    let small_config = create_small_config();
    let small_json = serde_json::to_string_pretty(&small_config).unwrap();
    fs::write(&small_path, small_json).unwrap();

    let large_path = temp_dir.path().join("large.json");
    let large_config = create_large_config();
    let large_json = serde_json::to_string_pretty(&large_config).unwrap();
    fs::write(&large_path, large_json).unwrap();

    let mut group = c.benchmark_group("json_load");

    group.bench_function("small_config", |b| {
        let loader = JsonLoader::new(&small_path);
        b.iter(|| {
            let config: SmallConfig = loader.load().unwrap();
            black_box(config);
        });
    });

    group.bench_function("large_config", |b| {
        let loader = JsonLoader::new(&large_path);
        b.iter(|| {
            let config: LargeConfig = loader.load().unwrap();
            black_box(config);
        });
    });

    group.finish();
}

fn bench_json_save(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let small_config = create_small_config();
    let large_config = create_large_config();

    let mut group = c.benchmark_group("json_save");

    group.bench_function("small_config", |b| {
        let path = temp_dir.path().join("small_save.json");
        let loader = JsonLoader::new(&path);
        b.iter(|| {
            loader.save(&small_config).unwrap();
        });
    });

    group.bench_function("large_config", |b| {
        let path = temp_dir.path().join("large_save.json");
        let loader = JsonLoader::new(&path);
        b.iter(|| {
            loader.save(&large_config).unwrap();
        });
    });

    group.finish();
}

fn bench_layered_merge(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();

    let mut group = c.benchmark_group("layered_merge");

    for num_layers in [2, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("override", num_layers),
            &num_layers,
            |b, &num_layers| {
                // Create multiple config files
                let mut layered: LayeredLoader<SmallConfig> =
                    LayeredLoader::new(MergeStrategy::Override);

                for i in 0..num_layers {
                    let path = temp_dir.path().join(format!("layer_{}.json", i));
                    let config = SmallConfig {
                        name: format!("layer_{}", i),
                        port: 8080 + i as u16,
                        enabled: i % 2 == 0,
                    };
                    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
                    layered = layered.add_layer(JsonLoader::new(&path));
                }

                b.iter(|| {
                    let config = layered.load().unwrap();
                    black_box(config);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("deep_merge", num_layers),
            &num_layers,
            |b, &num_layers| {
                let mut layered: LayeredLoader<SmallConfig> =
                    LayeredLoader::new(MergeStrategy::DeepMerge);

                for i in 0..num_layers {
                    let path = temp_dir.path().join(format!("layer_deep_{}.json", i));
                    let config = SmallConfig {
                        name: format!("layer_{}", i),
                        port: 8080 + i as u16,
                        enabled: i % 2 == 0,
                    };
                    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
                    layered = layered.add_layer(JsonLoader::new(&path));
                }

                b.iter(|| {
                    let config = layered.load().unwrap();
                    black_box(config);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_json_load,
    bench_json_save,
    bench_layered_merge
);
criterion_main!(benches);
