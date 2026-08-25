//! Benchmarks the actual per-frame hot path: `Scene::step()`, called once
//! per `tick_interval` by every real front-end (`pipes-app`,
//! `pipes-settings`'s live preview, `pipes-xscreensaver`). `default_config`
//! measures the shipped default; `large_scene` stress-tests a much bigger
//! grid and pipe count to see how the simulation scales, since a user can
//! turn both up via the settings app's "Grid size & reset" / "Pipe style &
//! count" panels.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pipes_core::{GridBounds, Scene, SimConfig};

fn bench_scene_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_step");

    group.bench_function("default_config", |b| {
        let mut scene = Scene::new(SimConfig::default(), 42);
        b.iter(|| black_box(scene.step()));
    });

    group.bench_function("large_scene", |b| {
        let config = SimConfig {
            bounds: GridBounds::new(64, 64, 64),
            max_pipes: 200,
            ..SimConfig::default()
        };
        let mut scene = Scene::new(config, 42);
        b.iter(|| black_box(scene.step()));
    });

    group.finish();
}

criterion_group!(benches, bench_scene_step);
criterion_main!(benches);
