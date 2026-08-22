struct Camera {
    view_proj: mat4x4<f32>,
    // .xyz is the world-space eye position; .w is unused padding (vec3
    // uniforms need 16-byte alignment in WGSL's address space rules).
    eye: vec4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );

    var out: VertexOutput;
    let world_pos = model_matrix * vec4<f32>(model.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    out.world_pos = world_pos.xyz;

    let normal_matrix = mat3x3<f32>(model_matrix[0].xyz, model_matrix[1].xyz, model_matrix[2].xyz);
    out.world_normal = normalize(normal_matrix * model.normal);
    out.color = instance.color;
    return out;
}

// Procedural environment reflection — a simple analytic sky (dark below
// the horizon, bright above it) sampled by a direction, standing in for a
// real cubemap texture. This project generates every mesh and every
// palette procedurally already (see docs/ARCHITECTURE.md); a hand-rolled
// gradient keeps the chrome material consistent with that — no texture
// asset, no image-loading dependency, no cubemap upload — while still
// giving pipes a real reflection instead of only a specular highlight.
fn sample_environment(dir: vec3<f32>) -> vec3<f32> {
    let sky_top = vec3<f32>(0.75, 0.85, 1.0);
    let sky_horizon = vec3<f32>(0.4, 0.42, 0.48);
    let ground = vec3<f32>(0.06, 0.06, 0.09);
    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    let below = mix(ground, sky_horizon, smoothstep(0.0, 0.35, t));
    return mix(below, sky_top, smoothstep(0.35, 1.0, t));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let key_light = normalize(vec3<f32>(0.5, 0.9, 0.35));
    let fill_light = normalize(vec3<f32>(-0.4, 0.2, -0.6));

    let key_diff = max(dot(n, key_light), 0.0);
    let fill_diff = max(dot(n, fill_light), 0.0) * 0.3;

    // The real per-pixel view direction (fragment toward the actual
    // orbiting eye), not an assumed constant — needed for a correct
    // reflection vector, unlike the old fixed-vector Blinn-Phong term this
    // replaces.
    let view_dir = normalize(camera.eye.xyz - in.world_pos);
    let reflect_dir = reflect(-view_dir, n);
    let env = sample_environment(reflect_dir);
    let glint = pow(max(dot(reflect_dir, key_light), 0.0), 80.0);

    let ambient = 0.16;
    let base_lit = in.color * (ambient + key_diff * 0.7 + fill_diff);
    // Reflection tinted toward the pipe's own color rather than a flat
    // grey mirror, so chrome shininess and the color palette read
    // together instead of the environment washing the palette out.
    let tinted_env = mix(env, env * in.color, 0.6);
    let lit = mix(base_lit, tinted_env, 0.4) + vec3<f32>(1.0) * glint * 0.7;
    return vec4<f32>(lit, 1.0);
}
