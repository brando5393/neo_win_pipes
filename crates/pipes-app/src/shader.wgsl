struct Camera {
    view_proj: mat4x4<f32>,
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

    let normal_matrix = mat3x3<f32>(model_matrix[0].xyz, model_matrix[1].xyz, model_matrix[2].xyz);
    out.world_normal = normalize(normal_matrix * model.normal);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let key_light = normalize(vec3<f32>(0.5, 0.9, 0.35));
    let fill_light = normalize(vec3<f32>(-0.4, 0.2, -0.6));

    let key_diff = max(dot(n, key_light), 0.0);
    let fill_diff = max(dot(n, fill_light), 0.0) * 0.3;

    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = normalize(key_light + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 40.0);

    let ambient = 0.22;
    let lit = in.color * (ambient + key_diff * 0.8 + fill_diff) + vec3<f32>(1.0, 1.0, 1.0) * spec * 0.6;
    return vec4<f32>(lit, 1.0);
}
