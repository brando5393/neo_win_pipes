//! Converts a `pipes_core::Scene` into per-mesh GPU instance data: one entry
//! per pipe segment (cylinder or cuboid, depending on `PipeStyle`) and one
//! per joint/cap (sphere, at varying scale for ball vs. elbow joints).

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use pipes_core::{Color, GridPos, JointKind, Pipe, PipeStyle, Scene};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct InstanceRaw {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub _pad: f32,
}

/// Tunables for how thick pipes/joints render, in grid units (one grid unit
/// == one cell == the distance between consecutive path points).
/// `#[serde(default)]` (container-level) makes every field individually
/// forward-compatible with older saved config files — see the matching
/// note on `pipes_core::SimConfig` for why this matters.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PipeVisuals {
    pub pipe_radius: f32,
    pub ball_joint_scale: f32,
    pub elbow_joint_scale: f32,
    pub cap_scale: f32,
    /// Size multiplier (relative to `pipe_radius`) for the rare teapot
    /// easter-egg joint (see `pipes_core::JointKind::Teapot`). Bigger than
    /// the ball/elbow scales since the teapot mesh needs to actually read
    /// as a teapot rather than a blob.
    pub teapot_scale: f32,
}

impl Default for PipeVisuals {
    fn default() -> Self {
        Self {
            pipe_radius: 0.18,
            ball_joint_scale: 1.4,
            // Bigger than a sphere-joint scale would need to be: this is
            // now the elbow *torus*'s major (bend) radius, and needs real
            // room beyond its own tube thickness (`geometry::elbow`'s fixed
            // 0.33 tube-ratio, applied in `renderer.rs`) to read as a
            // curved bend instead of a ball — verified by actually
            // rendering it (see CLAUDE.md's testing philosophy point 8).
            elbow_joint_scale: 3.0,
            cap_scale: 1.1,
            teapot_scale: 3.5,
        }
    }
}

#[derive(Default)]
pub struct InstanceSets {
    pub round_segments: Vec<InstanceRaw>,
    pub square_segments: Vec<InstanceRaw>,
    pub joints: Vec<InstanceRaw>,
    pub elbows: Vec<InstanceRaw>,
    pub teapots: Vec<InstanceRaw>,
}

fn to_vec3(p: GridPos) -> Vec3 {
    Vec3::new(p.x as f32, p.y as f32, p.z as f32)
}

fn color_array(c: Color) -> [f32; 3] {
    [c.r, c.g, c.b]
}

fn segment_instance(a: GridPos, b: GridPos, radius: f32, color: [f32; 3]) -> InstanceRaw {
    let a = to_vec3(a);
    let b = to_vec3(b);
    let mid = (a + b) * 0.5;
    let dir = (b - a).normalize();
    let rotation = Quat::from_rotation_arc(Vec3::Z, dir);
    let model =
        Mat4::from_scale_rotation_translation(Vec3::new(radius, radius, 1.0), rotation, mid);
    InstanceRaw {
        model: model.to_cols_array_2d(),
        color,
        _pad: 0.0,
    }
}

fn point_instance(p: GridPos, scale: f32, color: [f32; 3]) -> InstanceRaw {
    let model =
        Mat4::from_scale_rotation_translation(Vec3::splat(scale), Quat::IDENTITY, to_vec3(p));
    InstanceRaw {
        model: model.to_cols_array_2d(),
        color,
        _pad: 0.0,
    }
}

/// Rotation that carries `geometry::elbow`'s canonical tangent directions
/// (+Z, -X) onto this joint's actual incoming/outgoing pipe directions,
/// built by mapping one orthonormal basis onto another (third axis of each
/// via cross product, so both bases share the same handedness and the
/// result is a pure rotation, never a reflection). `d_in`/`d_out` must be
/// unit vectors and perpendicular — always true at a grid elbow, which is
/// by construction a 90-degree turn between two cardinal axes.
fn elbow_rotation(d_in: Vec3, d_out: Vec3) -> Quat {
    let c1 = Vec3::Z;
    let c2 = Vec3::NEG_X;
    let c3 = c1.cross(c2);
    let t1 = d_out;
    let t2 = -d_in;
    let t3 = t1.cross(t2);
    let canonical = Mat4::from_cols(
        c1.extend(0.0),
        c2.extend(0.0),
        c3.extend(0.0),
        Vec3::ZERO.extend(1.0),
    );
    let target = Mat4::from_cols(
        t1.extend(0.0),
        t2.extend(0.0),
        t3.extend(0.0),
        Vec3::ZERO.extend(1.0),
    );
    let rotation = target * canonical.transpose();
    Quat::from_mat4(&rotation)
}

fn elbow_instance(p: GridPos, scale: f32, rotation: Quat, color: [f32; 3]) -> InstanceRaw {
    let model = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, to_vec3(p));
    InstanceRaw {
        model: model.to_cols_array_2d(),
        color,
        _pad: 0.0,
    }
}

/// Builds this frame's instances from the scene's current pipes. When the
/// scene is dissolving (see `Scene::dissolve_progress`), every pipe/joint
/// shrinks proportionally toward zero as the countdown runs out — the
/// classic-inspired "dissolve away" transition, purely a render-time
/// effect (`pipes-core` doesn't know or care that geometry shrinks; it
/// only tracks the countdown).
pub fn build_instances(scene: &Scene, visuals: &PipeVisuals) -> InstanceSets {
    let mut sets = InstanceSets::default();
    let shrink = 1.0 - scene.dissolve_progress().unwrap_or(0.0);

    for pipe in scene.pipes() {
        push_pipe(pipe, visuals, shrink, &mut sets);
    }

    sets
}

fn push_pipe(pipe: &Pipe, visuals: &PipeVisuals, shrink: f32, sets: &mut InstanceSets) {
    let color = color_array(pipe.color);
    let path = pipe.path();
    let radius = visuals.pipe_radius * shrink;

    let segments = match pipe.style {
        PipeStyle::Round => &mut sets.round_segments,
        PipeStyle::Square => &mut sets.square_segments,
    };
    for pair in path.windows(2) {
        segments.push(segment_instance(pair[0], pair[1], radius, color));
    }

    for &(index, kind) in pipe.joints() {
        match kind {
            JointKind::Teapot => {
                let scale = visuals.teapot_scale * radius;
                sets.teapots.push(point_instance(path[index], scale, color));
            }
            JointKind::Ball => {
                let scale = visuals.ball_joint_scale * radius;
                sets.joints.push(point_instance(path[index], scale, color));
            }
            JointKind::Elbow if index > 0 => {
                // `pipe.joints()` records a joint before the following
                // point is appended (see `Pipe::step`), so `index + 1` is
                // always in bounds. `index == 0` is the one real exception,
                // guarded above: a pipe's very first step already counts as
                // a "turn" if it differs from the spawn's initial phantom
                // direction, with no real predecessor to bend from —
                // handled by the fallback arm below instead.
                let d_in = (to_vec3(path[index]) - to_vec3(path[index - 1])).normalize();
                let d_out = (to_vec3(path[index + 1]) - to_vec3(path[index])).normalize();
                let scale = visuals.elbow_joint_scale * radius;
                let rotation = elbow_rotation(d_in, d_out);
                sets.elbows
                    .push(elbow_instance(path[index], scale, rotation, color));
            }
            JointKind::Elbow => {
                // index == 0: no predecessor to bend from (see above) —
                // fall back to the same sphere used for Ball joints.
                let scale = visuals.elbow_joint_scale * radius;
                sets.joints.push(point_instance(path[index], scale, color));
            }
        }
    }

    if let Some(&start) = path.first() {
        sets.joints
            .push(point_instance(start, visuals.cap_scale * radius, color));
    }
    if path.len() > 1 {
        let end = *path.last().expect("checked non-empty above");
        sets.joints
            .push(point_instance(end, visuals.cap_scale * radius, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipes_core::{Direction, SimConfig};

    /// Approximates the uniform x/y (radius) scale baked into a model
    /// matrix built by `segment_instance`/`point_instance` — rotation
    /// preserves vector length, so the local x-basis column's length
    /// after transform equals the scale that was applied before rotation.
    fn approx_radius_scale(instance: &InstanceRaw) -> f32 {
        let col0 = Vec3::new(
            instance.model[0][0],
            instance.model[0][1],
            instance.model[0][2],
        );
        col0.length()
    }

    #[test]
    fn dissolve_shrink_scales_radius_proportionally() {
        let pipe = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(0, 0, 0),
            Direction::PosX,
        );
        let visuals = PipeVisuals::default();

        let mut full = InstanceSets::default();
        push_pipe(&pipe, &visuals, 1.0, &mut full);
        let mut half = InstanceSets::default();
        push_pipe(&pipe, &visuals, 0.5, &mut half);
        let mut gone = InstanceSets::default();
        push_pipe(&pipe, &visuals, 0.0, &mut gone);

        // A single fresh pipe only has its two end caps (no segments yet).
        let full_cap = approx_radius_scale(&full.joints[0]);
        let half_cap = approx_radius_scale(&half.joints[0]);
        let gone_cap = approx_radius_scale(&gone.joints[0]);

        assert!((full_cap - visuals.cap_scale * visuals.pipe_radius).abs() < 1e-5);
        assert!(
            (half_cap - full_cap * 0.5).abs() < 1e-5,
            "half shrink must halve the radius"
        );
        assert!(
            gone_cap < 1e-5,
            "zero shrink must collapse to (approximately) nothing"
        );
    }

    #[test]
    fn elbow_at_the_very_first_step_falls_back_to_a_sphere_instead_of_panicking() {
        // Regression test: `Pipe::step` can record a joint at path index 0
        // if the pipe's very first move already differs from its spawn
        // direction (see `Pipe::step`'s `self.joints.push((self.path.len()
        // - 1, joint))`, called before `self.path` grows past 1 element).
        // `push_pipe` originally assumed every joint had a real predecessor
        // and panicked with "attempt to subtract with overflow" reading
        // `path[index - 1]` — caught by actually running the settings app
        // and hitting the crash, not by reading the code (see CLAUDE.md's
        // testing philosophy point 8).
        use pipes_core::{GridBounds, OccupancyGrid};
        use rand::{rngs::StdRng, SeedableRng};

        let mut pipe = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(0, 0, 0),
            Direction::PosX,
        );
        let mut grid = OccupancyGrid::new(GridBounds::new(10, 10, 10));
        grid.occupy(GridPos::new(0, 0, 0));
        let mut rng = StdRng::seed_from_u64(1);
        // straight_weight: 0 forces any legal turn to win the weighted
        // pick; elbow_probability: 1.0 forces JointKind::Elbow specifically
        // (rather than Ball, which never hit this bug — see `push_pipe`).
        pipe.step(&mut grid, &mut rng, 0, 1, 1.0, 0.0, 100);
        assert_eq!(
            pipe.joints(),
            &[(0, JointKind::Elbow)],
            "test setup must actually produce the index-0 elbow this test targets"
        );

        let mut sets = InstanceSets::default();
        push_pipe(&pipe, &PipeVisuals::default(), 1.0, &mut sets);
        assert!(
            sets.elbows.is_empty(),
            "no real predecessor to bend from — must fall back to a sphere, not a torus"
        );
        assert_eq!(
            sets.joints.len(),
            3,
            "the index-0 fallback sphere plus the start and end caps"
        );
    }

    #[test]
    fn empty_scene_has_no_instances() {
        let scene = Scene::new(
            SimConfig {
                max_pipes: 0,
                ..SimConfig::default()
            },
            1,
        );
        let sets = build_instances(&scene, &PipeVisuals::default());
        assert!(sets.round_segments.is_empty());
        assert!(sets.square_segments.is_empty());
        assert!(sets.joints.is_empty());
    }

    #[test]
    fn teapot_joints_land_in_their_own_bucket_not_the_regular_joints_bucket() {
        // teapot_probability 1.0 makes every turn a teapot (see
        // pipes_core::pipe::tests for the roll logic itself) — here we
        // only need to confirm build_instances routes JointKind::Teapot
        // into `sets.teapots` rather than `sets.joints`.
        let mut scene = Scene::new(
            SimConfig {
                max_pipes: 1,
                bounds: pipes_core::GridBounds::new(20, 20, 20),
                straight_weight: 1,
                turn_weight: 50,
                teapot_easter_egg_enabled: true,
                teapot_probability: 1.0,
                ..SimConfig::default()
            },
            1,
        );
        for _ in 0..30 {
            scene.step();
        }
        let sets = build_instances(&scene, &PipeVisuals::default());
        assert!(
            !sets.teapots.is_empty(),
            "expected at least one teapot instance with teapot_probability 1.0 and heavy turning"
        );
    }

    #[test]
    fn one_step_pipe_produces_one_segment_and_two_caps() {
        let mut scene = Scene::new(
            SimConfig {
                max_pipes: 1,
                ..SimConfig::default()
            },
            1,
        );
        scene.step(); // spawn
        scene.step(); // grow by one
        let sets = build_instances(&scene, &PipeVisuals::default());
        let total_segments = sets.round_segments.len() + sets.square_segments.len();
        assert!(
            total_segments >= 1,
            "a growing pipe must produce at least one segment instance"
        );
        assert!(
            sets.joints.len() >= 2,
            "start and end caps must always be present"
        );
    }

    #[test]
    fn segment_instance_model_matrix_is_finite() {
        let inst = segment_instance(
            GridPos::new(0, 0, 0),
            GridPos::new(1, 0, 0),
            0.2,
            [1.0, 0.0, 0.0],
        );
        for row in inst.model {
            for v in row {
                assert!(v.is_finite());
            }
        }
    }

    #[test]
    fn segment_instance_handles_every_cardinal_direction_without_nan() {
        // Regression guard: Quat::from_rotation_arc(Z, -Z) is the classic
        // degenerate case for "rotate this axis onto that axis" — make sure
        // it doesn't produce NaNs for any of our six grid directions.
        for dir in Direction::ALL {
            let a = GridPos::new(5, 5, 5);
            let b = a.step(dir);
            let inst = segment_instance(a, b, 0.2, [1.0, 1.0, 1.0]);
            for row in inst.model {
                for v in row {
                    assert!(
                        v.is_finite(),
                        "direction {dir:?} produced a non-finite matrix entry"
                    );
                }
            }
        }
    }
}
