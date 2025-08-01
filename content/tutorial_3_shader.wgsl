struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>, // pixel coordinates
    @location(0) vert_pos: vec3<f32>, // position coordinates
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {

    const vertices: array<vec4<f32>, 3> = array(
        vec4<f32>(-0.5f, -0.5f, 0, 1),
        vec4<f32>(0.5f, -0.5f, 0, 1),
        vec4<f32>( 0.0f,  0.5f, 0, 1),
    );

    let pos = vertices[min(in_vertex_index, 2u)];
    return VertexOutput(pos, pos.xyz);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = in.vert_pos + 0.5;
    return vec4<f32>(r.xy, 0.0, 1.0);
}
