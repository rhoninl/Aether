use aether_renderer::gpu::mesh::Vertex;

/// Minimum valid subdivision count for plane generation.
const MIN_SUBDIVISIONS: u32 = 1;
/// Default number of sphere stacks.
const DEFAULT_SPHERE_STACKS: u32 = 16;
/// Default number of sphere sectors.
const DEFAULT_SPHERE_SECTORS: u32 = 32;

/// Generate a flat plane centered at origin on the XZ plane.
///
/// `size` is the half-extent (total width = size * 2).
/// `subdivisions` is the number of quads per axis.
/// Returns (vertices, indices).
pub fn generate_plane(size: f32, subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
    let subdivisions = subdivisions.max(MIN_SUBDIVISIONS);
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

/// Generate a unit cube centered at origin.
///
/// Each face has its own 4 vertices for correct normals.
/// Returns (vertices, indices).
pub fn generate_cube() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    // Face definitions: (normal, tangent_u, tangent_v)
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // +Y (top)
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        // -Y (bottom)
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        // +X (right)
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        // -X (left)
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        // +Z (front)
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        // -Z (back)
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];

    for (face_idx, (normal, u_dir, v_dir)) in faces.iter().enumerate() {
        let base = (face_idx * 4) as u32;

        // 4 corners: (-1,-1), (1,-1), (1,1), (-1,1) in face-local UV space
        let offsets: [(f32, f32, f32, f32); 4] = [
            (-1.0, -1.0, 0.0, 0.0),
            (1.0, -1.0, 1.0, 0.0),
            (1.0, 1.0, 1.0, 1.0),
            (-1.0, 1.0, 0.0, 1.0),
        ];

        for (ou, ov, tex_u, tex_v) in &offsets {
            let position = [
                normal[0] + u_dir[0] * ou + v_dir[0] * ov,
                normal[1] + u_dir[1] * ou + v_dir[1] * ov,
                normal[2] + u_dir[2] * ou + v_dir[2] * ov,
            ];
            vertices.push(Vertex {
                position,
                normal: *normal,
                uv: [*tex_u, *tex_v],
            });
        }

        // Two triangles per face
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);

        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    (vertices, indices)
}

/// Generate a UV sphere centered at origin with radius 1.
///
/// Returns (vertices, indices).
pub fn generate_sphere(stacks: u32, sectors: u32) -> (Vec<Vertex>, Vec<u32>) {
    let stacks = stacks.max(2);
    let sectors = sectors.max(3);

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
                position: [x, y, z],
                normal: [x, y, z],
                uv: [u, v],
            });
        }
    }

    for i in 0..stacks {
        for j in 0..sectors {
            let first = i * (sectors + 1) + j;
            let second = first + sectors + 1;

            if i != 0 {
                indices.push(first);
                indices.push(second);
                indices.push(first + 1);
            }
            if i != stacks - 1 {
                indices.push(first + 1);
                indices.push(second);
                indices.push(second + 1);
            }
        }
    }

    (vertices, indices)
}

/// Generate a UV sphere with default stacks and sectors.
pub fn generate_default_sphere() -> (Vec<Vertex>, Vec<u32>) {
    generate_sphere(DEFAULT_SPHERE_STACKS, DEFAULT_SPHERE_SECTORS)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Plane tests ----

    #[test]
    fn plane_vertex_count() {
        let (verts, _) = generate_plane(5.0, 4);
        // (4+1) * (4+1) = 25 vertices
        assert_eq!(verts.len(), 25);
    }

    #[test]
    fn plane_index_count() {
        let (_, indices) = generate_plane(5.0, 4);
        // 4 * 4 * 6 = 96 indices
        assert_eq!(indices.len(), 96);
    }

    #[test]
    fn plane_normals_point_up() {
        let (verts, _) = generate_plane(1.0, 2);
        for v in &verts {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        }
    }

    #[test]
    fn plane_uvs_in_range() {
        let (verts, _) = generate_plane(10.0, 8);
        for v in &verts {
            assert!(
                v.uv[0] >= 0.0 && v.uv[0] <= 1.0,
                "u out of range: {}",
                v.uv[0]
            );
            assert!(
                v.uv[1] >= 0.0 && v.uv[1] <= 1.0,
                "v out of range: {}",
                v.uv[1]
            );
        }
    }

    #[test]
    fn plane_centered_at_origin() {
        let (verts, _) = generate_plane(5.0, 2);
        // All Y should be 0
        for v in &verts {
            assert!((v.position[1]).abs() < f32::EPSILON);
        }
        // Should have corners at (-5, 0, -5) and (5, 0, 5)
        let min_x = verts.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        let max_x = verts.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        assert!((min_x + 5.0).abs() < f32::EPSILON);
        assert!((max_x - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn plane_min_subdivisions_clamped() {
        let (verts, indices) = generate_plane(1.0, 0);
        // Should clamp to 1 subdivision: 4 verts, 6 indices
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn plane_indices_in_bounds() {
        let (verts, indices) = generate_plane(5.0, 4);
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx, "index {idx} out of bounds (max {max_idx})");
        }
    }

    // ---- Cube tests ----

    #[test]
    fn cube_vertex_count() {
        let (verts, _) = generate_cube();
        assert_eq!(verts.len(), 24); // 6 faces * 4 vertices
    }

    #[test]
    fn cube_index_count() {
        let (_, indices) = generate_cube();
        assert_eq!(indices.len(), 36); // 6 faces * 2 triangles * 3
    }

    #[test]
    fn cube_normals_are_unit_vectors() {
        let (verts, _) = generate_cube();
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "normal not unit: len={len}");
        }
    }

    #[test]
    fn cube_has_six_distinct_normals() {
        let (verts, _) = generate_cube();
        let mut normals = std::collections::HashSet::new();
        for v in &verts {
            // Quantize to avoid float comparison issues
            let key = (
                (v.normal[0] * 10.0) as i32,
                (v.normal[1] * 10.0) as i32,
                (v.normal[2] * 10.0) as i32,
            );
            normals.insert(key);
        }
        assert_eq!(normals.len(), 6);
    }

    #[test]
    fn cube_indices_in_bounds() {
        let (verts, indices) = generate_cube();
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx);
        }
    }

    #[test]
    fn cube_uvs_in_range() {
        let (verts, _) = generate_cube();
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    // ---- Sphere tests ----

    #[test]
    fn sphere_vertex_count() {
        let (verts, _) = generate_sphere(8, 16);
        // (stacks+1) * (sectors+1) = 9 * 17 = 153
        assert_eq!(verts.len(), 153);
    }

    #[test]
    fn sphere_normals_are_unit_vectors() {
        let (verts, _) = generate_sphere(8, 16);
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal not unit: len={len}");
        }
    }

    #[test]
    fn sphere_positions_on_unit_sphere() {
        let (verts, _) = generate_sphere(8, 16);
        for v in &verts {
            let r = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!((r - 1.0).abs() < 1e-4, "position not on unit sphere: r={r}");
        }
    }

    #[test]
    fn sphere_indices_in_bounds() {
        let (verts, indices) = generate_sphere(8, 16);
        let max_idx = verts.len() as u32;
        for &idx in &indices {
            assert!(idx < max_idx, "index {idx} out of bounds (max {max_idx})");
        }
    }

    #[test]
    fn sphere_uvs_in_range() {
        let (verts, _) = generate_sphere(8, 16);
        for v in &verts {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
        }
    }

    #[test]
    fn sphere_clamps_min_params() {
        let (verts, indices) = generate_sphere(1, 2);
        // stacks clamped to 2, sectors clamped to 3
        assert_eq!(verts.len(), ((2 + 1) * (3 + 1)) as usize);
        assert!(!indices.is_empty());
    }

    #[test]
    fn sphere_has_top_and_bottom_poles() {
        let (verts, _) = generate_sphere(8, 16);
        let has_top = verts.iter().any(|v| (v.position[1] - 1.0).abs() < 1e-4);
        let has_bottom = verts.iter().any(|v| (v.position[1] + 1.0).abs() < 1e-4);
        assert!(has_top, "sphere missing top pole");
        assert!(has_bottom, "sphere missing bottom pole");
    }

    #[test]
    fn default_sphere_uses_expected_params() {
        let (verts, _) = generate_default_sphere();
        let expected = ((DEFAULT_SPHERE_STACKS + 1) * (DEFAULT_SPHERE_SECTORS + 1)) as usize;
        assert_eq!(verts.len(), expected);
    }
}
