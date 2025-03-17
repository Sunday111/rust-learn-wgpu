use cgmath::Vector3;
use wgpu::util::DeviceExt;

use crate::line_vertex::LineVertex;

pub struct LinesDrawPass {
    pub lines_pipeline: wgpu::RenderPipeline,
    pub lines_vertex_buffer: wgpu::Buffer,
    pub num_lines: u32,
}

impl LinesDrawPass {
    pub fn new(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        texture_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
        vertex_shader: &wgpu::ShaderModule,
        fragment_shader: &wgpu::ShaderModule,
    ) -> Self {
        let (lines_vertex_buffer, num_lines) = Self::make_lines_buffer(device);
        Self {
            lines_pipeline: Self::create_pipeline(
                device,
                camera_bind_group_layout,
                texture_format,
                depth_stencil_state,
                vertex_shader,
                fragment_shader,
            ),
            lines_vertex_buffer,
            num_lines,
        }
    }

    pub fn create_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        texture_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
        vertex_shader: &wgpu::ShaderModule,
        fragment_shader: &wgpu::ShaderModule,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lines Render Pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Lines Render Pipeline Layout"),
                    bind_group_layouts: &[&camera_bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill, // others require Features::NON_FILL_POLYGON_MODE
                unclipped_depth: false,                // Requires Features::DEPTH_CLIP_CONTROL
                conservative: false, // Requires Features::CONSERVATIVE_RASTERIZATION
            },
            vertex: wgpu::VertexState {
                module: vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            depth_stencil: depth_stencil_state.clone(),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass, camera_bind_group: &wgpu::BindGroup) {
        if self.num_lines != 0 {
            render_pass.set_pipeline(&self.lines_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.lines_vertex_buffer.slice(..));
            render_pass.draw(0..self.num_lines, 0..self.num_lines / 2);
        }
    }

    fn make_lines_buffer(device: &wgpu::Device) -> (wgpu::Buffer, u32) {
        let ranges: [(Vector3<f32>, Vector3<f32>, i32, [f32; 3]); 2] = [
            (Vector3::unit_x(), Vector3::unit_y(), 51, [1.0, 0.0, 0.0]),
            (Vector3::unit_y(), Vector3::unit_x(), 51, [0.0, 1.0, 0.0]),
        ];

        let vertices: Vec<LineVertex> = ranges
            .iter()
            .map(|(spread_direction, line_direction, num_lines, color)| {
                let h = num_lines / 2;
                let hf = h as f32;
                (-h..h)
                    .map(move |x| {
                        [
                            (x as f32) * spread_direction + line_direction * hf,
                            (x as f32) * spread_direction - line_direction * hf,
                        ]
                    })
                    .flatten()
                    .map(move |v| LineVertex {
                        position: v.into(),
                        color: *color,
                    })
            })
            .flatten()
            .collect();

        (
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            vertices.len() as u32,
        )
    }
}
