//! Procedural primitive mesh generators.
//!
//! All generators emit triangles wound CCW around their outward normal so
//! that wgpu's default `FrontFace::Ccw` + `Face::Back` culling keeps every
//! face. The `*_triangles_wind_outward` tests enforce this invariant.

use crate::gpu::mesh::Vertex;

/// Minimum subdivision count for plane generation.
const PLANE_MIN_SUBDIVISIONS: u32 = 1;
/// Minimum stacks for sphere generation.
const SPHERE_MIN_STACKS: u32 = 2;
/// Minimum sectors for sphere generation.
const SPHERE_MIN_SECTORS: u32 = 3;

/// Generate a flat plane on the XZ plane, centered at origin, facing +Y.
///
/// `size` is the half-extent (total edge length = `size * 2`). `subdivisions`
/// is the number of quads per axis and is clamped to at least 1.
pub fn generate_plane(size: f32, subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
    let subdivisions = subdivisions.max(PLANE_MIN_SUBDIVISIONS);
    let step = (size * 2.0) / subdivisions as f32;
    let vert_count = (subdivisions + 1) as usize;

    let mut vertices = Vec::with_capacity(vert_count * vert_count);
    let mut indices = Vec::with_capacity((subdivisions * subdivisions * 6) as usize);

    for z in 0..=subdivisions {
        for x in 0..=subdivisions {
            let px = -size + x as f32 * step;
            let pz = -size + z as f32 * step;
            let u = x as f32 / subdivisions as f32;
            let v = z as f32 / subdivisions as f32;
            vertices.push(Vertex {
                position: [px, 0.0, pz],
                normal: [0.0, 1.0, 0.0],
                uv: [u, v],
            });
        }
    }

    for z in 0..subdivisions {
        for x in 0..subdivisions {
            let top_left = z * (subdivisions + 1) + x;
            let top_right = top_left + 1;
            let bottom_left = (z + 1) * (subdivisions + 1) + x;
            let bottom_right = bottom_left + 1;

            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    (vertices, indices)
}

/// Generate a cube centered at origin with per-face normals (24 vertices, 36 indices).
///
/// `size` is the half-extent of the cube (total edge length = `size * 2`).
pub fn generate_cube(size: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    // Face definitions: (normal, u_dir, v_dir) where `u_dir × v_dir == normal`
    // is the invariant that keeps the CCW triangle winding below outward-facing.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];

    for (face_idx, (normal, u_dir, v_dir)) in faces.iter().enumerate() {
        let base = (face_idx * 4) as u32;

        let offsets: [(f32, f32, f32, f32); 4] = [
            (-1.0, -1.0, 0.0, 0.0),
            (1.0, -1.0, 1.0, 0.0),
            (1.0, 1.0, 1.0, 1.0),
            (-1.0, 1.0, 0.0, 1.0),
        ];

        for (ou, ov, tex_u, tex_v) in &offsets {
            let position = [
                (normal[0] + u_dir[0] * ou + v_dir[0] * ov) * size,
                (normal[1] + u_dir[1] * ou + v_dir[1] * ov) * size,
                (normal[2] + u_dir[2] * ou + v_dir[2] * ov) * size,
            ];
            vertices.push(Vertex {
                position,
                normal: *normal,
                uv: [*tex_u, *tex_v],
            });
        }

        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);

        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    (vertices, indices)
}

/// Generate a UV sphere centered at origin.
///
/// `radius` scales the sphere. `stacks` is the number of horizontal rings
/// (clamped to at least 2) and `sectors` is the number of vertical slices
/// (clamped to at least 3).
pub fn generate_sphere(radius: f32, stacks: u32, sectors: u32) -> (Vec<Vertex>, Vec<u32>) {
    let stacks = stacks.max(SPHERE_MIN_STACKS);
    let sectors = sectors.max(SPHERE_MIN_SECTORS);

    let mut vertices = Vec::with_capacity(((stacks + 1) * (sectors + 1)) as usize);
    let mut indices = Vec::new();

    let pi = std::f32::consts::PI;
    let two_pi = 2.0 * pi;

    for i in 0..=stacks {
        let stack_angle = pi / 2.0 - (i as f32) * pi / (stacks as f32);
        let xy = stack_angle.cos();
        let y = stack_angle.sin();

        for j in 0..=sectors {
            let sector_angle = (j as f32) * two_pi / (sectors as f32);
            let x = xy * sector_angle.cos();
            let z = xy * sector_angle.sin();

            let u = j as f32 / sectors as f32;
            let v = i as f32 / stacks as f32;

            vertices.push(Vertex {
                position: [x * radius, y * radius, z * radius],
                normal: [x, y, z],
                uv: [u, v],
            });
        }
    }

    // CCW winding when viewed from outside: index order is upper-left →
    // upper-right → lower-left for the top triangle of each quad, and
    // upper-right → lower-right → lower-left for the bottom triangle.
    for i in 0..stacks {
        for j in 0..sectors {
            let first = i * (sectors + 1) + j;
            let second = first + sectors + 1;

            if i != 0 {
                indices.push(first);
                indices.push(first + 1);
                indices.push(second);
            }
            if i != stacks - 1 {
                indices.push(first + 1);
                indices.push(second + 1);
                indices.push(second);
            }
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert every triangle's `(v1 - v0) × (v2 - v0)` points into the same
    /// hemisphere as the vertex normal — i.e. survives back-face culling.
    fn assert_triangles_wind_outward(verts: &[Vertex], indices: &[u32]) {
        assert_eq!(indices.len() % 3, 0, "index buffer not a multiple of 3");
        for tri in indices.chunks_exact(3) {
            let v0 = verts[tri[0] as usize].position;
            let v1 = verts[tri[1] as usize].position;
            let v2 = verts[tri[2] as usize].position;
            let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let cross = [
                edge1[1] * edge2[2] - edge1[2] * edge2[1],
                edge1[2] * edge2[0] - edge1[0] * edge2[2],
                edge1[0] * edge2[1] - edge1[1] * edge2[0],
            ];
            let n = verts[tri[0] as usize].normal;
            let dot = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2];
            assert!(
                dot > 0.0,
                "triangle {tri:?} winds inward: cross={cross:?} normal={n:?}",
            );
        }
    }

    // ---- Plane ----

    #[test]
    fn plane_vertex_count() {
        let (verts, _) = generate_plane(5.0, 4);
        assert_eq!(verts.len(), 25); // (4+1)²
    }

    #[test]
    fn plane_index_count() {
        let (_, idx) = generate_plane(5.0, 4);
        assert_eq!(idx.len(), 96); // 4² quads × 6
    }

    #[test]
    fn plane_normals_point_up() {
        let (verts, _) = generate_plane(1.0, 2);
        for v in &verts {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        }
    }

    #[test]
    fn plane_indices_in_bounds() {
        let (verts, idx) = generate_plane(5.0, 4);
        let max = verts.len() as u32;
        for &i in &idx {
            assert!(i < max);
        }
    }

    #[test]
    fn plane_uvs_in_range() {
        let (verts, _) = generate_plane(10.0, 8);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn plane_subdivisions_clamped_to_one() {
        let (verts, idx) = generate_plane(1.0, 0);
        assert_eq!(verts.len(), 4);
        assert_eq!(idx.len(), 6);
    }

    #[test]
    fn plane_centered_at_origin() {
        let (verts, _) = generate_plane(5.0, 2);
        let min_x = verts.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        let max_x = verts.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        assert!((min_x + 5.0).abs() < 1e-6);
        assert!((max_x - 5.0).abs() < 1e-6);
    }

    #[test]
    fn plane_triangles_wind_outward() {
        let (verts, idx) = generate_plane(5.0, 4);
        assert_triangles_wind_outward(&verts, &idx);
    }

    // ---- Cube ----

    #[test]
    fn cube_vertex_count() {
        let (verts, _) = generate_cube(1.0);
        assert_eq!(verts.len(), 24);
    }

    #[test]
    fn cube_index_count() {
        let (_, idx) = generate_cube(1.0);
        assert_eq!(idx.len(), 36);
    }

    #[test]
    fn cube_normals_are_unit_vectors() {
        let (verts, _) = generate_cube(1.0);
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn cube_has_six_distinct_normals() {
        let (verts, _) = generate_cube(1.0);
        let mut set = std::collections::HashSet::new();
        for v in &verts {
            set.insert((
                (v.normal[0] * 10.0) as i32,
                (v.normal[1] * 10.0) as i32,
                (v.normal[2] * 10.0) as i32,
            ));
        }
        assert_eq!(set.len(), 6);
    }

    #[test]
    fn cube_indices_in_bounds() {
        let (verts, idx) = generate_cube(1.0);
        let max = verts.len() as u32;
        for &i in &idx {
            assert!(i < max);
        }
    }

    #[test]
    fn cube_uvs_in_range() {
        let (verts, _) = generate_cube(1.0);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn cube_size_scales_positions() {
        let (unit, _) = generate_cube(1.0);
        let (big, _) = generate_cube(3.0);
        for (a, b) in unit.iter().zip(big.iter()) {
            assert!((b.position[0] - a.position[0] * 3.0).abs() < 1e-5);
            assert!((b.position[1] - a.position[1] * 3.0).abs() < 1e-5);
            assert!((b.position[2] - a.position[2] * 3.0).abs() < 1e-5);
        }
    }

    #[test]
    fn cube_triangles_wind_outward() {
        let (verts, idx) = generate_cube(1.0);
        assert_triangles_wind_outward(&verts, &idx);
    }

    // ---- Sphere ----

    #[test]
    fn sphere_vertex_count() {
        let (verts, _) = generate_sphere(1.0, 8, 16);
        assert_eq!(verts.len(), ((8 + 1) * (16 + 1)) as usize);
    }

    #[test]
    fn sphere_normals_are_unit_vectors() {
        let (verts, _) = generate_sphere(1.0, 8, 16);
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn sphere_positions_on_surface() {
        let (verts, _) = generate_sphere(2.0, 8, 16);
        for v in &verts {
            let r = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!((r - 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn sphere_indices_in_bounds() {
        let (verts, idx) = generate_sphere(1.0, 8, 16);
        let max = verts.len() as u32;
        for &i in &idx {
            assert!(i < max);
        }
    }

    #[test]
    fn sphere_uvs_in_range() {
        let (verts, _) = generate_sphere(1.0, 8, 16);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn sphere_clamps_min_params() {
        let (verts, idx) = generate_sphere(1.0, 1, 2);
        assert_eq!(verts.len(), ((2 + 1) * (3 + 1)) as usize);
        assert!(!idx.is_empty());
    }

    #[test]
    fn sphere_has_top_and_bottom_poles() {
        let (verts, _) = generate_sphere(1.0, 8, 16);
        let has_top = verts.iter().any(|v| (v.position[1] - 1.0).abs() < 1e-4);
        let has_bottom = verts.iter().any(|v| (v.position[1] + 1.0).abs() < 1e-4);
        assert!(has_top);
        assert!(has_bottom);
    }

    #[test]
    fn sphere_triangles_wind_outward() {
        let (verts, idx) = generate_sphere(1.0, 8, 16);
        assert_triangles_wind_outward(&verts, &idx);
    }
}
