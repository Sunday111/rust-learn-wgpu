use cgmath::{Deg, Transform};
use klgl::Rotator;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

fn transform_model(vertices: &mut [Vertex]) {
    let rm = Rotator {
        yaw: Deg(0.0),
        pitch: Deg(0.0),
        roll: Deg(0.0),
    }
    .to_matrix();

    for v in vertices {
        v.position = rm.transform_point(v.position.into()).into();
    }
}

const TRIANGLE_VERTICES: [Vertex; 3] = [
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
        tex_coords: [0.5, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
        tex_coords: [0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 1.0],
    },
];

const TRIANGLE_INDICES: &[u16] = &[0, 1, 2];

const HEX_VERTICES: [Vertex; 5] = [
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [1.0; 3],
        tex_coords: [0.4131759, 0.99240386],
    }, // A
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [1.0; 3],
        tex_coords: [0.0048659444, 0.56958647],
    }, // B
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [1.0; 3],
        tex_coords: [0.28081453, 0.05060294],
    }, // C
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [1.0; 3],
        tex_coords: [0.85967, 0.1526709],
    }, // D
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [1.0; 3],
        tex_coords: [0.9414737, 0.7347359],
    }, // E
];

const HEX_INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
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
            ],
        }
    }
}

pub struct ModelsDrawPass {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    pub instances_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub textures: [wgpu::BindGroup; 2],
    pub active_texture: u32,
}

impl ModelsDrawPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("model_draw_pass_texture_bind_group_layout"),
            });

        let models_pipeline = ModelsDrawPass::create_render_pipeline(
            &device,
            &camera_bind_group_layout,
            &texture_bind_group_layout,
            surface_format,
            depth_stencil_state,
        );

        let mut model_instances: Vec<Instance> = vec![];
        Self::compute_model_instances(&mut model_instances, Deg(45.0));

        let model_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&model_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let mut tri_vert: [Vertex; 3] = TRIANGLE_VERTICES.into();
        transform_model(&mut tri_vert);

        let model_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&tri_vert),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let num_indices = TRIANGLE_INDICES.len();
        let model_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(TRIANGLE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let textures = {
            [
                {
                    let diffuse_texture = klgl::Texture::from_bytes(
                        &device,
                        &queue,
                        tutorial_embedded_content::HAPPY_TREE_PNG,
                        "happy-tree.png",
                    )
                    .unwrap();
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &texture_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                            },
                        ],
                        label: Some("happy tree bind group"),
                    })
                },
                {
                    let diffuse_texture = klgl::Texture::from_bytes(
                        &device,
                        &queue,
                        tutorial_embedded_content::ILLUMINATI_PNG,
                        "illuminati.png",
                    )
                    .unwrap();

                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &texture_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                            },
                        ],
                        label: Some("illuminati bind group"),
                    })
                },
            ]
        };

        Self {
            pipeline: models_pipeline,
            vertex_buffer: model_vertex_buffer,
            index_buffer: model_index_buffer,
            instances: model_instances,
            instances_buffer: model_instances_buffer,
            num_indices: num_indices as u32,
            textures,
            active_texture: 0,
        }
    }

    fn compute_model_instances(v: &mut Vec<Instance>, angle: Deg<f32>) {
        const NUM_INSTANCES_PER_ROW: u32 = 10;
        v.clear();
        v.extend((0..NUM_INSTANCES_PER_ROW).flat_map(|y| {
            (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                let rotation = Rotator {
                    yaw: angle * (-0.5 + ((x + 1) as f32 / NUM_INSTANCES_PER_ROW as f32)),
                    pitch: angle * (-0.5 + ((y + 1) as f32 / NUM_INSTANCES_PER_ROW as f32)),
                    roll: Deg(0.0),
                };

                Instance {
                    model: (cgmath::Matrix4::from_translation(cgmath::Vector3 {
                        x: x as f32,
                        y: y as f32,
                        z: 1.0,
                    }) * rotation.to_matrix())
                    .into(),
                }
            })
        }));
    }

    pub fn update_model_instances(&mut self, queue: &wgpu::Queue, angle: Deg<f32>) {
        Self::compute_model_instances(&mut self.instances, angle);
        queue.write_buffer(
            &self.instances_buffer,
            0,
            bytemuck::cast_slice(&self.instances[..]),
        );
    }

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Model Shader"),
            source: wgpu::ShaderSource::Wgsl(tutorial_embedded_content::TUTORIAL_7_SHADER.into()),
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Triangle Strip Render Pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Triangle Strip Render Pipeline Layout"),
                    bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
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

    pub fn swap_model(&mut self, device: &wgpu::Device) {
        let (vertices, indices) = {
            if self.num_indices == TRIANGLE_INDICES.len() as u32 {
                let mut hex_vert: [Vertex; 5] = HEX_VERTICES.into();
                transform_model(&mut hex_vert);
                (hex_vert.to_vec(), HEX_INDICES)
            } else {
                let mut tri_vert: [Vertex; 3] = TRIANGLE_VERTICES.into();
                transform_model(&mut tri_vert);
                (tri_vert.to_vec(), TRIANGLE_INDICES)
            }
        };

        self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.num_indices = indices.len() as u32;
    }

    pub fn set_active_texture(&mut self, index: u32) {
        self.active_texture = index.min(1);
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass, camera_bind_group: &wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.textures[self.active_texture as usize], &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instances_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..self.instances.len() as _);
    }
}
