use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cgmath::Deg;
use klgl::Rotator;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const TRIANGLE_VERTICES: [Vertex; 3] = [
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
    },
];

fn make_hollow_triangle() -> (Vec<Vertex>, Vec<u16>) {
    (
        TRIANGLE_VERTICES
            .iter()
            .copied()
            .chain(TRIANGLE_VERTICES.iter().map(|x| Vertex {
                position: x.position.map(|x| x * 0.3),
                color: x.color,
            }))
            .collect(),
        vec![0, 3, 2, 5, 1, 4, 0, 4, 3],
    )
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
    color_seed: [f32; 3],
}

impl Instance {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in the shader.
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials, we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5, not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct ModelsDrawPass {
    pub pipelines: Vec<wgpu::RenderPipeline>,
    pub pipeline_idx: u16,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    pub instances_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl ModelsDrawPass {
    pub async fn new(
        render_context: Rc<RefCell<klgl::RenderContext>>,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let pipelines = {
            let ctx = render_context.borrow();
            let gamma_correction = !ctx.config.format.is_srgb();
            let mut dss = depth_stencil_state.unwrap();
            [wgpu::CompareFunction::Less, wgpu::CompareFunction::Always]
                .iter()
                .map(|f| {
                    dss.depth_compare = *f;
                    ModelsDrawPass::create_render_pipeline(
                        &ctx.device,
                        &camera_bind_group_layout,
                        ctx.config.format,
                        Some(dss.clone()),
                        gamma_correction,
                    )
                })
                .collect()
        };

        let mut instances: Vec<Instance> = vec![];
        Self::compute_model_instances(&mut instances);

        let model_instances_buffer =
            render_context
                .borrow()
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let (vertices, indices) = make_hollow_triangle();

        let vertex_buffer =
            render_context
                .borrow()
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let num_indices = indices.len();
        let index_buffer =
            render_context
                .borrow()
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

        Self {
            pipelines,
            pipeline_idx: 0,
            vertex_buffer,
            index_buffer,
            instances,
            instances_buffer: model_instances_buffer,
            num_indices: num_indices as u32,
        }
    }

    fn compute_model_instances(v: &mut Vec<Instance>) {
        v.clear();
        v.push(Instance {
            model: (cgmath::Matrix4::from_translation(cgmath::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }) * Rotator {
                yaw: Deg(0.0),
                pitch: Deg(0.0),
                roll: Deg(0.0),
            }
            .to_matrix()
                * cgmath::Matrix4::from_scale(10.0))
            .into(),
            color_seed: [1.0, 0.5, 0.25],
        });
        v.push(Instance {
            model: (cgmath::Matrix4::from_translation(cgmath::Vector3 {
                x: 0.0,
                y: 3.0,
                z: 1.0,
            }) * Rotator {
                yaw: Deg(0.0),
                pitch: Deg(90.0),
                roll: Deg(180.0),
            }
            .to_matrix()
                * cgmath::Matrix4::from_scale(10.0))
            .into(),
            color_seed: [0.25, 0.5, 1.0],
        });
    }

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
        gamma_correction: bool,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Model Shader"),
            source: wgpu::ShaderSource::Wgsl(tutorial_embedded_content::TUTORIAL_8_SHADER.into()),
        });

        let mut constants: HashMap<String, f64> = HashMap::new();
        constants.insert(
            "enable_gamma_correction".into(),
            if gamma_correction { 1.0 } else { 0.0 },
        );

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Triangle Strip Render Pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Triangle Strip Render Pipeline Layout"),
                    bind_group_layouts: &[&camera_bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), Instance::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
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

    pub fn toggle_depth(&mut self) {
        self.pipeline_idx = (self.pipeline_idx + 1) % self.pipelines.len() as u16;
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass, camera_bind_group: &wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipelines[self.pipeline_idx as usize]);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instances_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..self.instances.len() as _);
    }
}
