//! Procedural mesh generation for pipe segments and joints. Pure functions
//! (no GPU handle needed), so shape correctness — vertex/index counts,
//! finite/normalized normals, bounds — is unit-testable without a window.
//!
//! All meshes are generated in local space, centered at the origin, built
//! along the local +Z axis where relevant (segments run from -0.5 to +0.5
//! along Z); the renderer's instance transform handles scale/rotation/
//! translation into world space.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

/// A capped cylinder of the given radius, running along local Z from -0.5 to
/// +0.5 (unit length before the instance transform scales it). Used for
/// round pipe segments.
pub fn cylinder(radius: f32, sides: u32) -> Mesh {
    debug_assert!(sides >= 3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..sides {
        let theta = (i as f32 / sides as f32) * std::f32::consts::TAU;
        let (sin, cos) = theta.sin_cos();
        let normal = [cos, sin, 0.0];
        vertices.push(Vertex {
            position: [cos * radius, sin * radius, -0.5],
            normal,
        });
        vertices.push(Vertex {
            position: [cos * radius, sin * radius, 0.5],
            normal,
        });
    }
    for i in 0..sides {
        let a = (i * 2) as u16;
        let b = ((i + 1) % sides * 2) as u16;
        push_quad(&mut indices, a, a + 1, b + 1, b);
    }

    Mesh { vertices, indices }
}

/// An axis-aligned box, half-extent `radius` in X/Y and running local Z from
/// -0.5 to +0.5. Used for square pipe segments.
pub fn cuboid(radius: f32) -> Mesh {
    let r = radius;
    // 6 faces, 4 unique vertices each (flat-shaded normals), 24 verts total.
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [1.0, 0.0, 0.0],
            [[r, -r, -0.5], [r, r, -0.5], [r, r, 0.5], [r, -r, 0.5]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-r, r, -0.5], [-r, -r, -0.5], [-r, -r, 0.5], [-r, r, 0.5]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[r, r, -0.5], [-r, r, -0.5], [-r, r, 0.5], [r, r, 0.5]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-r, -r, -0.5], [r, -r, -0.5], [r, -r, 0.5], [-r, -r, 0.5]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[-r, -r, 0.5], [r, -r, 0.5], [r, r, 0.5], [-r, r, 0.5]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[r, -r, -0.5], [-r, -r, -0.5], [-r, r, -0.5], [r, r, -0.5]],
        ),
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (normal, corners) in faces {
        let base = vertices.len() as u16;
        for corner in corners {
            vertices.push(Vertex {
                position: corner,
                normal,
            });
        }
        push_quad(&mut indices, base, base + 1, base + 2, base + 3);
    }
    Mesh { vertices, indices }
}

/// A UV sphere of the given radius, centered at the origin. Used for pipe
/// joints (both ball and elbow joints, at different scales — see
/// `renderer::instances`).
pub fn sphere(radius: f32, lat_segments: u32, lon_segments: u32) -> Mesh {
    debug_assert!(lat_segments >= 2 && lon_segments >= 3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for lat in 0..=lat_segments {
        let theta = (lat as f32 / lat_segments as f32) * std::f32::consts::PI;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for lon in 0..=lon_segments {
            let phi = (lon as f32 / lon_segments as f32) * std::f32::consts::TAU;
            let (sin_phi, cos_phi) = phi.sin_cos();
            let normal = [sin_theta * cos_phi, cos_theta, sin_theta * sin_phi];
            vertices.push(Vertex {
                position: [normal[0] * radius, normal[1] * radius, normal[2] * radius],
                normal,
            });
        }
    }

    let stride = lon_segments + 1;
    for lat in 0..lat_segments {
        for lon in 0..lon_segments {
            let a = (lat * stride + lon) as u16;
            let b = (lat * stride + lon + 1) as u16;
            let c = ((lat + 1) * stride + lon + 1) as u16;
            let d = ((lat + 1) * stride + lon) as u16;
            push_quad(&mut indices, a, b, c, d);
        }
    }

    Mesh { vertices, indices }
}

fn push_quad(indices: &mut Vec<u16>, a: u16, b: u16, c: u16, d: u16) {
    indices.extend_from_slice(&[a, b, c, a, c, d]);
}

/// Revolves a 2D meridian profile — `(axial position along Y, radius)`
/// pairs, ordered bottom to top — around the Y axis, generalizing
/// `sphere()` to arbitrary silhouettes. Vertex normals are the true
/// surface normal derived from the profile's local slope (averaged
/// across adjacent segments at interior nodes), not a naive radial
/// approximation, so lit shading looks correct even on sharply-tapering
/// profiles. Used to build the teapot's body and spout below.
pub fn lathe(profile: &[(f32, f32)], lon_segments: u32) -> Mesh {
    debug_assert!(profile.len() >= 2 && lon_segments >= 3);
    let n = profile.len();

    // `profile` entries are `(y, r)`, matching the doc comment. Named
    // explicitly here (rather than destructured as `(a, b)`) after a real
    // bug where the destructuring order silently didn't match that
    // convention, swapping every radius/axial value.
    let segment_normal_2d = |p0: (f32, f32), p1: (f32, f32)| -> (f32, f32) {
        let (y0, r0) = p0;
        let (y1, r1) = p1;
        let (dy, dr) = (y1 - y0, r1 - r0);
        let len = (dr * dr + dy * dy).sqrt().max(1e-6);
        (dy / len, -dr / len)
    };

    let mut node_normals_2d: Vec<(f32, f32)> = Vec::with_capacity(n);
    for i in 0..n {
        let mut acc = (0.0f32, 0.0f32);
        if i > 0 {
            let (nr, ny) = segment_normal_2d(profile[i - 1], profile[i]);
            acc.0 += nr;
            acc.1 += ny;
        }
        if i + 1 < n {
            let (nr, ny) = segment_normal_2d(profile[i], profile[i + 1]);
            acc.0 += nr;
            acc.1 += ny;
        }
        let len = (acc.0 * acc.0 + acc.1 * acc.1).sqrt().max(1e-6);
        node_normals_2d.push((acc.0 / len, acc.1 / len));
    }

    let mut vertices = Vec::new();
    for (i, &(y, r)) in profile.iter().enumerate() {
        let (nr, ny) = node_normals_2d[i];
        for lon in 0..=lon_segments {
            let phi = (lon as f32 / lon_segments as f32) * std::f32::consts::TAU;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(Vertex {
                position: [r * cos_phi, y, r * sin_phi],
                normal: [nr * cos_phi, ny, nr * sin_phi],
            });
        }
    }

    let stride = lon_segments + 1;
    let mut indices = Vec::new();
    for ring in 0..(n as u32 - 1) {
        for lon in 0..lon_segments {
            let a = (ring * stride + lon) as u16;
            let b = (ring * stride + lon + 1) as u16;
            let c = ((ring + 1) * stride + lon + 1) as u16;
            let d = ((ring + 1) * stride + lon) as u16;
            push_quad(&mut indices, a, b, c, d);
        }
    }

    Mesh { vertices, indices }
}

/// A partial torus arc: sweeps a tube of `minor_radius` along a circular
/// path of `major_radius` lying in the XZ plane, from angle 0 to
/// `arc_radians` (not necessarily a full turn). Used for the teapot's
/// handle. The two open ends of the arc are left uncapped — acceptable
/// since they end up embedded against the body from ordinary viewing
/// angles.
pub fn torus_arc(
    major_radius: f32,
    minor_radius: f32,
    arc_radians: f32,
    major_segments: u32,
    minor_segments: u32,
) -> Mesh {
    debug_assert!(major_segments >= 2 && minor_segments >= 3);
    let mut vertices = Vec::new();
    for i in 0..=major_segments {
        let theta = (i as f32 / major_segments as f32) * arc_radians;
        let (sin_t, cos_t) = theta.sin_cos();
        let center = [cos_t * major_radius, 0.0, sin_t * major_radius];
        let out = [cos_t, 0.0, sin_t]; // unit vector, radially outward from the Y axis
        for j in 0..=minor_segments {
            let phi = (j as f32 / minor_segments as f32) * std::f32::consts::TAU;
            let (sin_p, cos_p) = phi.sin_cos();
            let normal = [cos_p * out[0], sin_p, cos_p * out[2]];
            vertices.push(Vertex {
                position: [
                    center[0] + normal[0] * minor_radius,
                    center[1] + normal[1] * minor_radius,
                    center[2] + normal[2] * minor_radius,
                ],
                normal,
            });
        }
    }

    let stride = minor_segments + 1;
    let mut indices = Vec::new();
    for i in 0..major_segments {
        for j in 0..minor_segments {
            let a = (i * stride + j) as u16;
            let b = (i * stride + j + 1) as u16;
            let c = ((i + 1) * stride + j + 1) as u16;
            let d = ((i + 1) * stride + j) as u16;
            push_quad(&mut indices, a, b, c, d);
        }
    }

    Mesh { vertices, indices }
}

/// Concatenates several sub-meshes into one, transforming each by its
/// given model matrix first (positions fully transformed; normals
/// rotated via the inverse-transpose, correct even under the non-uniform
/// scales the teapot's parts use).
pub fn merge_meshes(parts: &[(Mesh, Mat4)]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (mesh, transform) in parts {
        let base = vertices.len() as u16;
        let normal_matrix = transform.inverse().transpose();
        for v in &mesh.vertices {
            let pos = transform.transform_point3(Vec3::from(v.position));
            let normal = normal_matrix
                .transform_vector3(Vec3::from(v.normal))
                .normalize();
            vertices.push(Vertex {
                position: pos.into(),
                normal: normal.into(),
            });
        }
        for &i in &mesh.indices {
            indices.push(base + i);
        }
    }
    Mesh { vertices, indices }
}

/// A procedural teapot silhouette (lathed body, tapered spout, torus-arc
/// handle, small lid knob) for the classic screensaver's teapot easter
/// egg — see `docs/RESEARCH.md`. **Not** the exact historical Utah
/// teapot control-point dataset: reproducing that faithfully needs
/// precise published control-point data this project didn't have on hand
/// to copy correctly, so rather than guess at numbers from memory and
/// risk a subtly-wrong "classic" model, this is an honest procedural
/// approximation aiming for the same unmistakably-a-teapot silhouette.
/// Centered near the origin (like the other joint meshes) at roughly
/// unit scale before the instance transform scales it further.
pub fn teapot() -> Mesh {
    let body_profile = [
        (0.0_f32, 0.02_f32),
        (0.05, 0.28),
        (0.18, 0.46),
        (0.35, 0.50),
        (0.55, 0.44),
        (0.75, 0.30),
        (0.88, 0.16),
        (0.95, 0.10),
        (1.0, 0.02),
    ];
    let body = lathe(&body_profile, 20);

    // Regression note: an earlier version used a full 1.0-unit-long spout
    // profile (matching the body's own height!) translated out past the
    // body's surface — the spout alone ended up spanning x in roughly
    // [0.55, 1.43], nearly 3x the body's 0.5 radius, reading as a long
    // flat bar rather than a teapot spout. Caught only by rendering the
    // teapot and looking at it (unit tests check well-formedness, not
    // proportions) — see CLAUDE.md's testing philosophy point 8. Kept
    // short here and based near the body's actual surface instead.
    // Chunkier than a first pass at these numbers: at a glance from normal
    // viewing distance (this is a small, rare joint decoration, not a
    // close-up hero prop), a too-thin spout/handle is indistinguishable
    // from a plain ball joint with a stray pixel on it. Sized to read
    // clearly as "spout" and "handle" silhouettes while staying within
    // teapot_is_well_formed_and_roughly_teapot_sized's aspect-ratio guard.
    let spout_profile = [(0.0_f32, 0.12_f32), (0.24, 0.09), (0.48, 0.05)];
    let spout = lathe(&spout_profile, 10);
    let spout_transform = Mat4::from_translation(Vec3::new(0.40, 0.42, 0.0))
        * Mat4::from_rotation_z(-60f32.to_radians());

    let handle = torus_arc(0.20, 0.055, 210f32.to_radians(), 16, 8);
    let handle_transform = Mat4::from_translation(Vec3::new(-0.55, 0.5, 0.0))
        * Mat4::from_rotation_z(90f32.to_radians());

    let knob = sphere(0.12, 6, 8);
    let knob_transform = Mat4::from_translation(Vec3::new(0.0, 1.03, 0.0));

    let mut mesh = merge_meshes(&[
        (body, Mat4::IDENTITY),
        (spout, spout_transform),
        (handle, handle_transform),
        (knob, knob_transform),
    ]);

    // The body profile spans y in [0, 1]; recenter on its vertical middle
    // so the teapot is placed like our other joint meshes (centered near
    // the origin), rather than sitting entirely above it.
    for v in &mut mesh.vertices {
        v.position[1] -= 0.5;
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_well_formed(mesh: &Mesh) {
        assert!(!mesh.vertices.is_empty(), "mesh must have vertices");
        assert!(!mesh.indices.is_empty(), "mesh must have indices");
        assert_eq!(
            mesh.indices.len() % 3,
            0,
            "indices must form whole triangles"
        );
        for &i in &mesh.indices {
            assert!((i as usize) < mesh.vertices.len(), "index {i} out of range");
        }
        for v in &mesh.vertices {
            for c in v.position.iter().chain(v.normal.iter()) {
                assert!(c.is_finite(), "non-finite vertex component: {c}");
            }
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal not unit length: {len}");
        }
    }

    #[test]
    fn cylinder_is_well_formed() {
        let mesh = cylinder(0.2, 12);
        assert_well_formed(&mesh);
        assert_eq!(mesh.vertices.len(), 24); // 2 rings * 12 sides
        assert_eq!(mesh.indices.len(), 12 * 6); // 12 side quads * 2 tris * 3 idx
    }

    #[test]
    fn cuboid_is_well_formed() {
        let mesh = cuboid(0.2);
        assert_well_formed(&mesh);
        assert_eq!(mesh.vertices.len(), 24); // 6 faces * 4 verts, flat-shaded
        assert_eq!(mesh.indices.len(), 6 * 6); // 6 faces * 2 tris * 3 idx
    }

    #[test]
    fn sphere_is_well_formed() {
        let mesh = sphere(0.25, 8, 12);
        assert_well_formed(&mesh);
        assert_eq!(mesh.vertices.len() as u32, (8 + 1) * (12 + 1));
    }

    #[test]
    fn lathe_is_well_formed() {
        let profile = [(0.0_f32, 0.1_f32), (0.5, 0.4), (1.0, 0.05)];
        let mesh = lathe(&profile, 12);
        assert_well_formed(&mesh);
        assert_eq!(mesh.vertices.len(), 3 * 13); // 3 rings * (12 lon + 1 seam)
    }

    #[test]
    fn lathe_straight_cylinder_matches_cylinder_radius() {
        // A 2-point profile with constant radius is just a cylinder in
        // disguise — every vertex should sit exactly on that radius, and
        // the normals should be purely radial (no vertical component),
        // cross-checking `lathe`'s slope-derived normals against the
        // known-correct case where the slope is zero.
        let mesh = lathe(&[(0.0_f32, 0.3_f32), (1.0, 0.3)], 16);
        assert_well_formed(&mesh);
        for v in &mesh.vertices {
            let r = (v.position[0].powi(2) + v.position[2].powi(2)).sqrt();
            assert!((r - 0.3).abs() < 1e-4, "expected radius 0.3, got {r}");
            assert!(
                v.normal[1].abs() < 1e-4,
                "a straight wall's normal must have no vertical component"
            );
        }
    }

    #[test]
    fn torus_arc_is_well_formed() {
        let mesh = torus_arc(0.2, 0.05, std::f32::consts::PI, 10, 8);
        assert_well_formed(&mesh);
        assert_eq!(mesh.vertices.len(), 11 * 9); // (10 major + 1) * (8 minor + 1)
    }

    #[test]
    fn torus_arc_tube_stays_within_major_plus_minor_radius() {
        let (major, minor) = (0.2, 0.05);
        for v in torus_arc(major, minor, std::f32::consts::TAU, 12, 8).vertices {
            let r = (v.position[0].powi(2) + v.position[2].powi(2)).sqrt();
            assert!(r <= major + minor + 1e-4);
            assert!(v.position[1].abs() <= minor + 1e-4);
        }
    }

    #[test]
    fn merge_meshes_concatenates_and_offsets_indices_correctly() {
        let a = cuboid(0.1);
        let b = cuboid(0.1);
        let a_len = a.vertices.len();
        let merged = merge_meshes(&[
            (a, Mat4::IDENTITY),
            (b, Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))),
        ]);
        assert_well_formed(&merged);
        assert_eq!(merged.vertices.len(), a_len * 2);
        // The second cuboid's indices must be offset past the first's
        // vertex range, not aliasing back into it.
        assert!(merged
            .indices
            .iter()
            .skip(merged.indices.len() / 2)
            .all(|&i| i as usize >= a_len));
    }

    #[test]
    fn teapot_is_well_formed_and_roughly_teapot_sized() {
        let mesh = teapot();
        assert_well_formed(&mesh);
        // Loose sanity bounds, not a precise silhouette check: nothing
        // should be wildly far from the origin (a transform/composition
        // bug would tend to fling a sub-part's vertices way off, not
        // subtly).
        for v in &mesh.vertices {
            let dist =
                (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!(
                dist < 2.0,
                "teapot vertex implausibly far from origin: {dist}"
            );
        }

        // Regression guard for a real bug: an earlier spout was a full
        // 1.0-unit-long cone translated out past the body's own surface,
        // so the *overall shape* stretched to ~2 units wide (x in roughly
        // [-0.585, 1.43]) while staying under 1.2 tall/deep — a long flat
        // bar, not a teapot — even though every individual vertex still
        // passed the "not wildly far from the origin" check above. Only
        // caught by actually rendering it and looking (see CLAUDE.md's
        // testing philosophy point 8), not by any per-vertex distance
        // check. Guard the actual bounding-box aspect ratio instead: a
        // teapot silhouette (body + a modest spout/handle/knob) should be
        // roughly as wide as it is tall, not markedly elongated.
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &mesh.vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(v.position[axis]);
                max[axis] = max[axis].max(v.position[axis]);
            }
        }
        let extents = Vec3::from(max) - Vec3::from(min);
        assert!(
            extents.x < 1.5 * extents.y && extents.z < 1.5 * extents.y,
            "teapot bounding box is implausibly elongated relative to its \
             height — extents={extents:?} (min={min:?}, max={max:?})"
        );
    }

    #[test]
    fn round_meshes_stay_within_declared_radius() {
        for radius in [0.1, 0.2, 0.5] {
            for mesh in [cylinder(radius, 10), sphere(radius, 6, 8)] {
                for v in &mesh.vertices {
                    let r = (v.position[0].powi(2) + v.position[1].powi(2)).sqrt();
                    assert!(
                        r <= radius + 1e-4,
                        "vertex escaped declared radius: {r} > {radius}"
                    );
                }
            }
        }
    }

    #[test]
    fn cuboid_stays_within_declared_half_extent() {
        // cuboid's `radius` param is a per-axis half-extent, not a Euclidean
        // radius, so corners at (±r, ±r) are expected — check each axis
        // independently instead of the combined radius used for round meshes.
        for radius in [0.1, 0.2, 0.5] {
            for v in cuboid(radius).vertices {
                assert!(v.position[0].abs() <= radius + 1e-4);
                assert!(v.position[1].abs() <= radius + 1e-4);
            }
        }
    }
}
