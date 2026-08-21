//! Procedural mesh generation for pipe segments and joints. Pure functions
//! (no GPU handle needed), so shape correctness — vertex/index counts,
//! finite/normalized normals, bounds — is unit-testable without a window.
//!
//! All meshes are generated in local space, centered at the origin, built
//! along the local +Z axis where relevant (segments run from -0.5 to +0.5
//! along Z); the renderer's instance transform handles scale/rotation/
//! translation into world space.

use bytemuck::{Pod, Zeroable};

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
