use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cgmath::Deg;
use klgl::{Rotator, file_loader::FileDataHandle};
use wgpu::util::DeviceExt;

use crate::model::{ModelVertex, Vertex};

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
            // We need to switch from using a step mode of ModelVertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in the shader.
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials, we'll
                    // be using 2, 3, and 4, for ModelVertex. We'll start at slot 5, not conflict with them later
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
    ctx: Rc<RefCell<klgl::RenderContext>>,
    pipeline: wgpu::RenderPipeline,
    instances: Vec<Instance>,
    instances_buffer: wgpu::Buffer,
    loading_model: Option<LoadingModel>,
    model: Option<crate::model::Model>,
}

struct LoadingModel {
    endpoint: klgl::file_loader::FileLoaderEndpoint,
    received: HashMap<String, FileDataHandle>,
    remaining: u16,
    obj_path: String,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl LoadingModel {
    pub fn new(
        file_loader: &mut klgl::file_loader::FileLoader,
        obj_path: &str,
        bind_group_layout: wgpu::BindGroupLayout,
        requirements: &[&str],
    ) -> Self {
        let mut endpoint = file_loader.make_endpoint();
        let remaining = (requirements.len() as u16) + 1;
        endpoint.request(obj_path);
        for requirement in requirements {
            endpoint.request(&requirement);
        }

        Self {
            endpoint,
            obj_path: obj_path.into(),
            remaining,
            received: HashMap::new(),
            bind_group_layout,
        }
    }

    pub fn ready(&self) -> bool {
        self.remaining == 0
    }

    pub fn update(&mut self) {
        while let Ok(file_handle) = self.endpoint.receiver.try_recv() {
            let path = self.endpoint.loader.path_by_id(file_handle.id).unwrap();
            self.received.insert(path, file_handle);
            if self.remaining > 0 {
                self.remaining -= 1;
            }
        }
    }

    pub fn get(&self, ctx: &klgl::RenderContext) -> Option<anyhow::Result<crate::model::Model>> {
        if !self.ready() {
            return None;
        }

        Some(crate::model::load_model(
            &self.obj_path,
            &self.received,
            ctx,
            &self.bind_group_layout,
        ))
    }
}

impl ModelsDrawPass {
    pub async fn new(
        file_loader: &mut klgl::file_loader::FileLoader,
        render_context: Rc<RefCell<klgl::RenderContext>>,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let texture_bind_group_layout = {
            let ctx = render_context.borrow();
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                })
        };

        let models_pipeline = {
            let ctx = render_context.borrow();
            ModelsDrawPass::create_render_pipeline(
                &ctx.device,
                &camera_bind_group_layout,
                &texture_bind_group_layout,
                ctx.config.format,
                depth_stencil_state,
            )
        };

        let mut model_instances: Vec<Instance> = vec![];
        Self::compute_model_instances(&mut model_instances, Deg(45.0));

        let model_instances_buffer =
            render_context
                .borrow()
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&model_instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        // let model_path = "models/cube/cube.obj";
        // let model_requirements = [
        //     "models/cube/cube.mtl",
        //     "models/cube/cube-diffuse.jpg",
        //     "models/cube/cube-normal.png",
        // ];

        let model_path = "models/wooden_crate/wooden_crate.obj";
        let model_requirements = [
            "models/wooden_crate/wooden_crate.mtl",
            "models/wooden_crate/wooden_crate_base_color.png",
            "models/wooden_crate/wooden_crate_metallic.png",
            "models/wooden_crate/wooden_crate_normal.png",
            "models/wooden_crate/wooden_crate_roughness.png",
        ];

        let loading_model = Some(LoadingModel::new(
            &mut file_loader.clone(),
            model_path,
            texture_bind_group_layout.clone(),
            &model_requirements,
        ));

        Self {
            ctx: render_context,
            pipeline: models_pipeline,
            instances: model_instances,
            instances_buffer: model_instances_buffer,
            loading_model,
            model: None,
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

                let scale = cgmath::Matrix4::from_scale(0.1);

                Instance {
                    model: (cgmath::Matrix4::from_translation(cgmath::Vector3 {
                        x: (x as f32),
                        y: (y as f32),
                        z: 1.0,
                    }) * rotation.to_matrix()
                        * scale)
                        .into(),
                }
            })
        }));
    }

    pub fn update(&mut self, angle: Deg<f32>) {
        if let Some(loading_model) = &mut self.loading_model {
            loading_model.update();
            self.model = match loading_model.get(&self.ctx.borrow_mut()) {
                Some(model_result) => match model_result {
                    Ok(model) => {
                        log::info!("Model successfully loaded: {}", loading_model.obj_path);
                        self.loading_model = None;
                        Some(model)
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to load model {}. Error: {}",
                            loading_model.obj_path,
                            err
                        );
                        self.loading_model = None;
                        None
                    }
                },
                None => None,
            }
        }

        Self::compute_model_instances(&mut self.instances, angle);
        self.ctx.borrow().queue.write_buffer(
            &self.instances_buffer,
            0,
            bytemuck::cast_slice(&self.instances[..]),
        );
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        depth_stencil_state: Option<wgpu::DepthStencilState>,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Model Shader"),
            source: wgpu::ShaderSource::Wgsl(tutorial_embedded_content::TUTORIAL_9_SHADER.into()),
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
                buffers: &[ModelVertex::layout(), Instance::layout()],
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
                topology: wgpu::PrimitiveTopology::TriangleList,
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

    pub fn swap_model(&mut self) {}

    pub fn render(&self, render_pass: &mut wgpu::RenderPass, camera_bind_group: &wgpu::BindGroup) {
        if let Some(model) = &self.model {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(1, self.instances_buffer.slice(..));
            model.draw_instanced(
                render_pass,
                camera_bind_group,
                0..self.instances.len() as u32,
            );
        }
    }
}
