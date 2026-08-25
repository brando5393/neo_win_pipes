//! Benchmarks `build_instances`, which every real front-end rebuilds from
//! scratch every single frame (see its own doc comment in `instance.rs`) —
//! unlike the GPU-side buffers, this is pure CPU work and directly competes
//! with the rest of a frame's budget. `default_config` measures a scene
//! populated under the shipped default settings; `large_scene` stress-tests
//! a much bigger grid/pipe count, matching `pipes-core`'s own
//! `scene_step` benchmark so the two can be compared.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pipes_core::{GridBounds, Scene, SimConfig};
use pipes_render::{build_instances, PipeVisuals};

/// Steps a fresh scene forward until it's reasonably full of pipes, so the
/// benchmark measures realistic mid-simulation instance counts rather than
/// an empty or just-spawned scene.
fn populated_scene(config: SimConfig, seed: u64, warmup_ticks: u32) -> Scene {
    let mut scene = Scene::new(config, seed);
    for _ in 0..warmup_ticks {
        scene.step();
    }
    scene
}

fn bench_build_instances(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_instances");
    let visuals = PipeVisuals::default();

    group.bench_function("default_config", |b| {
        let scene = populated_scene(SimConfig::default(), 42, 200);
        b.iter(|| black_box(build_instances(&scene, &visuals)));
    });

    group.bench_function("large_scene", |b| {
        let config = SimConfig {
            bounds: GridBounds::new(64, 64, 64),
            max_pipes: 200,
            ..SimConfig::default()
        };
        let scene = populated_scene(config, 42, 400);
        b.iter(|| black_box(build_instances(&scene, &visuals)));
    });

    group.finish();
}

criterion_group!(benches, bench_build_instances);
criterion_main!(benches);
