struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>, // pixel coordinates
    @location(0) vert_pos: vec3<f32>, // position coordinates
    @location(1) vertex_index: u32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    const vertices: array<vec4<f32>, 4> = array(
        vec4<f32>(-1, -1, 0, 1),
        vec4<f32>(1, -1, 0, 1),
        vec4<f32>(-1, 1, 0, 1),
        vec4<f32>(1, 1, 0, 1),
    );

    out.clip_position = vertices[min(in_vertex_index, 3u)];
    out.vert_pos = out.clip_position.xyz;
    out.vertex_index = in_vertex_index;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = (in.vert_pos + 1) / 2;
    return vec4<f32>(r.xy, 0.0, 1.0); // See raw values
}
