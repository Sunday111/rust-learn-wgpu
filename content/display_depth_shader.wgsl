// Vertex shader

@group(0) @binding(0)
var t_depth: texture_depth_2d;
@group(0) @binding(1)
var s_depth: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    out.uv = (model.position + 1) / 2;
    out.uv.y = 1 - out.uv.y;
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let near = 0.1;
    let far = 100.0;
    let depth = textureSampleLevel(t_depth, s_depth, in.uv, 0);
    let r = (2.0 * near) / (far + near - depth * (far - near));
    return vec4<f32>(vec3<f32>(r), 1.0);

    //let depth = textureSampleLevel(t_depth, s_depth, in.uv, 0);
    //return vec4<f32>(vec3<f32>(depth), 1.0);
}
