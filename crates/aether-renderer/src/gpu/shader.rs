/// WGSL source for the PBR vertex + fragment shader.
pub const PBR_SHADER_SOURCE: &str = r#"
// ---- Bind group 0: Camera ----
struct CameraUniforms {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    view_position: vec4<f32>,  // xyz = position, w = padding
};
@group(0) @binding(0) var<uniform> camera: CameraUniforms;

// ---- Bind group 1: Model (per-instance) ----
struct ModelUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model: ModelUniforms;

// ---- Bind group 2: Material ----
struct MaterialUniforms {
    albedo: vec4<f32>,
    metallic_roughness: vec4<f32>,  // x = metallic, y = roughness, zw = padding
    emissive: vec4<f32>,            // xyz = emissive, w = padding
};
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
@group(2) @binding(1) var t_albedo: texture_2d<f32>;
@group(2) @binding(2) var s_albedo: sampler;

// ---- Bind group 3: Light + Shadows ----
struct LightUniforms {
    direction: vec4<f32>,     // xyz = direction, w = padding
    color: vec4<f32>,         // xyz = color, w = intensity
    cascade_vp_0: mat4x4<f32>,
    cascade_vp_1: mat4x4<f32>,
    cascade_vp_2: mat4x4<f32>,
    cascade_vp_3: mat4x4<f32>,
    cascade_splits: vec4<f32>,
};
@group(3) @binding(0) var<uniform> light: LightUniforms;
@group(3) @binding(1) var t_shadow: texture_depth_2d_array;
@group(3) @binding(2) var s_shadow: sampler_comparison;

// ---- Vertex I/O ----
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) view_depth: f32,
};

// ---- Vertex Shader ----
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.projection * camera.view * world_pos;
    out.world_normal = normalize((model.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    let view_pos = camera.view * world_pos;
    out.view_depth = -view_pos.z;
    return out;
}

// ---- Helper: select cascade index ----
fn select_cascade(depth: f32) -> i32 {
    if depth < light.cascade_splits.x {
        return 0;
    } else if depth < light.cascade_splits.y {
        return 1;
    } else if depth < light.cascade_splits.z {
        return 2;
    }
    return 3;
}

// ---- Helper: compute shadow factor ----
fn compute_shadow(world_pos: vec3<f32>, depth: f32) -> f32 {
    let cascade = select_cascade(depth);
    var light_vp: mat4x4<f32>;
    if cascade == 0 {
        light_vp = light.cascade_vp_0;
    } else if cascade == 1 {
        light_vp = light.cascade_vp_1;
    } else if cascade == 2 {
        light_vp = light.cascade_vp_2;
    } else {
        light_vp = light.cascade_vp_3;
    }
    let light_clip = light_vp * vec4<f32>(world_pos, 1.0);
    let light_ndc = light_clip.xyz / light_clip.w;
    let shadow_uv = vec2<f32>(
        light_ndc.x * 0.5 + 0.5,
        1.0 - (light_ndc.y * 0.5 + 0.5),
    );
    let shadow_depth = light_ndc.z;

    // Out of bounds = fully lit
    if shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 {
        return 1.0;
    }

    // Percentage-closer filtering (2x2)
    let texel_size = 1.0 / 2048.0;
    var shadow = 0.0;
    for (var y = -1; y <= 1; y += 2) {
        for (var x = -1; x <= 1; x += 2) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompareLevel(
                t_shadow,
                s_shadow,
                shadow_uv + offset,
                cascade,
                shadow_depth - 0.002,
            );
        }
    }
    return shadow / 4.0;
}

// ---- PBR helpers ----
const PI: f32 = 3.14159265359;

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d + 0.0001);
}

fn geometry_schlick(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k + 0.0001);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// ---- Fragment Shader ----
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo_tex = textureSample(t_albedo, s_albedo, in.uv);
    let albedo = material.albedo.rgb * albedo_tex.rgb;
    let metallic = material.metallic_roughness.x;
    let roughness = material.metallic_roughness.y;

    let n = normalize(in.world_normal);
    let v = normalize(camera.view_position.xyz - in.world_position);
    let l = normalize(-light.direction.xyz);
    let h = normalize(v + l);

    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_h = max(dot(n, h), 0.0);
    let h_dot_v = max(dot(h, v), 0.0);

    // PBR: Cook-Torrance specular
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let d = distribution_ggx(n_dot_h, roughness);
    let g = geometry_schlick(n_dot_v, roughness) * geometry_schlick(n_dot_l, roughness);
    let f = fresnel_schlick(h_dot_v, f0);

    let specular = (d * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
    let k_d = (vec3<f32>(1.0) - f) * (1.0 - metallic);
    let diffuse = k_d * albedo / PI;

    let light_color = light.color.rgb * light.color.w;

    // Shadow
    let shadow = compute_shadow(in.world_position, in.view_depth);

    let lo = (diffuse + specular) * light_color * n_dot_l * shadow;

    // Ambient
    let ambient = vec3<f32>(0.03) * albedo;

    // Emissive
    let emissive = material.emissive.rgb;

    let color = ambient + lo + emissive;

    // Tone mapping (Reinhard)
    let mapped = color / (color + vec3<f32>(1.0));

    // Gamma correction
    let gamma = pow(mapped, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(gamma, 1.0);
}
"#;

/// WGSL source for the shadow pass (depth-only).
pub const SHADOW_SHADER_SOURCE: &str = r#"
struct LightVP {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> light_vp: LightVP;

struct ModelUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model: ModelUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_shadow(in: VertexInput) -> @builtin(position) vec4<f32> {
    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    return light_vp.view_proj * world_pos;
}
"#;

/// Compiled shader module names.
pub const PBR_SHADER_LABEL: &str = "aether-pbr-shader";
pub const SHADOW_SHADER_LABEL: &str = "aether-shadow-shader";

/// Create a wgpu ShaderModule from WGSL source.
pub fn create_shader_module(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbr_shader_source_is_not_empty() {
        assert!(!PBR_SHADER_SOURCE.is_empty());
        assert!(PBR_SHADER_SOURCE.contains("vs_main"));
        assert!(PBR_SHADER_SOURCE.contains("fs_main"));
    }

    #[test]
    fn shadow_shader_source_is_not_empty() {
        assert!(!SHADOW_SHADER_SOURCE.is_empty());
        assert!(SHADOW_SHADER_SOURCE.contains("vs_shadow"));
    }

    #[test]
    fn shader_labels_are_distinct() {
        assert_ne!(PBR_SHADER_LABEL, SHADOW_SHADER_LABEL);
    }

    #[test]
    fn pbr_shader_has_required_bind_groups() {
        // Verify all 4 bind groups are referenced
        assert!(PBR_SHADER_SOURCE.contains("@group(0)"));
        assert!(PBR_SHADER_SOURCE.contains("@group(1)"));
        assert!(PBR_SHADER_SOURCE.contains("@group(2)"));
        assert!(PBR_SHADER_SOURCE.contains("@group(3)"));
    }

    #[test]
    fn pbr_shader_has_pbr_functions() {
        assert!(PBR_SHADER_SOURCE.contains("distribution_ggx"));
        assert!(PBR_SHADER_SOURCE.contains("geometry_schlick"));
        assert!(PBR_SHADER_SOURCE.contains("fresnel_schlick"));
    }

    #[test]
    fn shadow_shader_has_required_bind_groups() {
        assert!(SHADOW_SHADER_SOURCE.contains("@group(0)"));
        assert!(SHADOW_SHADER_SOURCE.contains("@group(1)"));
    }

    #[test]
    fn pbr_shader_has_shadow_sampling() {
        assert!(PBR_SHADER_SOURCE.contains("compute_shadow"));
        assert!(PBR_SHADER_SOURCE.contains("select_cascade"));
        assert!(PBR_SHADER_SOURCE.contains("textureSampleCompareLevel"));
    }

    #[test]
    fn pbr_shader_has_tone_mapping() {
        // Verify Reinhard tone mapping is present
        assert!(PBR_SHADER_SOURCE.contains("Reinhard"));
    }

    #[test]
    fn pbr_shader_vertex_attributes() {
        assert!(PBR_SHADER_SOURCE.contains("@location(0) position: vec3<f32>"));
        assert!(PBR_SHADER_SOURCE.contains("@location(1) normal: vec3<f32>"));
        assert!(PBR_SHADER_SOURCE.contains("@location(2) uv: vec2<f32>"));
    }
}
