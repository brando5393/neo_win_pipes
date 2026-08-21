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
}

impl Default for PipeVisuals {
    fn default() -> Self {
        Self {
            pipe_radius: 0.18,
            ball_joint_scale: 1.4,
            elbow_joint_scale: 1.05,
            cap_scale: 1.1,
        }
    }
}

#[derive(Default)]
pub struct InstanceSets {
    pub round_segments: Vec<InstanceRaw>,
    pub square_segments: Vec<InstanceRaw>,
    pub joints: Vec<InstanceRaw>,
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
        let scale = match kind {
            JointKind::Ball => visuals.ball_joint_scale,
            JointKind::Elbow => visuals.elbow_joint_scale,
        } * radius;
        sets.joints.push(point_instance(path[index], scale, color));
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
