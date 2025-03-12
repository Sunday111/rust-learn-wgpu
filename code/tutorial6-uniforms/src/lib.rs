use cgmath::{Deg, Transform, Vector3};
use std::{iter, pin::Pin};
use web_time::Instant;

use pollster::FutureExt;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod model_vertex;
use model_vertex::ModelVertex;

mod line_vertex;
use line_vertex::LineVertex;

use klgl::{Camera, CameraController, CameraUniform, Rotator};

#[cfg(not(target_arch = "wasm32"))]
use env_logger::Env;

const TRIANGLE_VERTICES: [ModelVertex; 3] = [
    ModelVertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
        tex_coords: [0.5, 0.0],
    },
    ModelVertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
        tex_coords: [0.0, 1.0],
    },
    ModelVertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 1.0],
    },
];

const TRIANGLE_INDICES: &[u16] = &[0, 1, 2];

const HEX_VERTICES: [ModelVertex; 5] = [
    ModelVertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [1.0; 3],
        tex_coords: [0.4131759, 0.99240386],
    }, // A
    ModelVertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [1.0; 3],
        tex_coords: [0.0048659444, 0.56958647],
    }, // B
    ModelVertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [1.0; 3],
        tex_coords: [0.28081453, 0.05060294],
    }, // C
    ModelVertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [1.0; 3],
        tex_coords: [0.85967, 0.1526709],
    }, // D
    ModelVertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [1.0; 3],
        tex_coords: [0.9414737, 0.7347359],
    }, // E
];

const HEX_INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

struct TextureState {
    bind_group: wgpu::BindGroup,
}

struct Renderer<'a> {
    start_time: Instant,
    window: Pin<Box<Window>>,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    clear_color: wgpu::Color,
    surface_configured: bool,
    frame_counter: klgl::FpsCounter,
    last_printed_fps: Instant,

    lines_pipeline: wgpu::RenderPipeline,
    lines_vertex_buffer: wgpu::Buffer,
    num_lines: u32,

    models_pipeline: wgpu::RenderPipeline,
    model_vertex_buffer: wgpu::Buffer,
    model_index_buffer: wgpu::Buffer,
    num_model_indices: u32,
    textures: [TextureState; 2],
    active_texture: u32,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,
}

struct App<'a> {
    renderer: Option<Renderer<'a>>,
}

impl<'a> App<'a> {
    fn new() -> Self {
        Self { renderer: None }
    }
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let renderer = Renderer::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        )
        .block_on();

        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match &mut self.renderer {
            Some(s) => s.window_event(event_loop, window_id, event),
            _ => {}
        }
    }
}

fn transform_model(vertices: &mut [ModelVertex]) {
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

impl<'a> Renderer<'a> {
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

    async fn new(w: Window) -> Self {
        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let window_box = Box::pin(w);
        // SAFETY: `boxed` is pinned, so we can safely create a reference to `window`
        let window_ref: &'static Window =
            unsafe { &*(Pin::as_ref(&window_box).get_ref() as *const _) };
        let size = window_ref.inner_size();

        let surface = instance.create_surface(window_ref).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    required_limits: if cfg!(target_arch = "wasm32") {
                        let mut l = wgpu::Limits::downlevel_webgl2_defaults();
                        l.max_texture_dimension_2d = 4096;
                        l
                    } else {
                        wgpu::Limits::default()
                    },
                    memory_hints: Default::default(),
                },
                // Some(&std::path::Path::new("trace")), // Trace path
                None,
            )
            .await
            .unwrap();

        let device_limits = device.limits();
        log::info!("device limits: {:?}", device_limits);

        let adapter_info = adapter.get_info();
        log::info!("adapter info: {:?}", adapter_info);

        #[cfg(target_arch = "wasm32")]
        {
            // Winit prevents sizing with CSS, so we have to set
            // the size manually when on web.
            use winit::platform::web::WindowExtWebSys;
            web_sys::window()
                .and_then(|win| win.document())
                .and_then(|doc| {
                    let dst = doc.get_element_by_id("wasm-body")?;
                    let canvas = web_sys::Element::from(window_ref.canvas()?);
                    dst.append_child(&canvas).ok()?;
                    Some(())
                })
                .expect("Couldn't append canvas to document body.");
        }

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an Srgb surface texture. Using a different
        // one will result all the colors comming out darker. If you want to support non
        // Srgb surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

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
                label: Some("texture_bind_group_layout"),
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        let camera = Camera::new(
            // position the camera 1 unit up and 2 units back
            // +z is out of the screen
            (-1.780416, -2.3149111, 4.5232105).into(),
            // have it look at the origin
            Rotator {
                yaw: Deg(36.0),
                pitch: Deg(29.0),
                roll: Deg(0.0),
            },
            config.width as f32 / config.height as f32,
            90.0,
            0.1,
            100.0,
        );

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let colored_vertices_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Solid Color Shader"),
            source: wgpu::ShaderSource::Wgsl(tutorial_content::COLORED_VERTICES_SHADER.into()),
        });

        let lines_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                module: &colored_vertices_shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &colored_vertices_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let models_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Model Shader"),
            source: wgpu::ShaderSource::Wgsl(tutorial_content::TUTORIAL_6_SHADER.into()),
        });

        let models_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Triangle Strip Render Pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Triangle Strip Render Pipeline Layout"),
                    bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: wgpu::VertexState {
                module: &models_shader,
                entry_point: Some("vs_main"),
                buffers: &[ModelVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &models_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let (lines_vertex_buffer, num_lines) = Renderer::make_lines_buffer(&device);

        let mut tri_vert: [ModelVertex; 3] = TRIANGLE_VERTICES.into();
        transform_model(&mut tri_vert);

        let model_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&tri_vert),
            usage: wgpu::BufferUsages::VERTEX,
        });

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
                        tutorial_content::HAPPY_TREE_PNG,
                        "happy-tree.png",
                    )
                    .unwrap();
                    TextureState {
                        bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &texture_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(
                                        &diffuse_texture.view,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &diffuse_texture.sampler,
                                    ),
                                },
                            ],
                            label: Some("happy tree bind group"),
                        }),
                    }
                },
                {
                    let diffuse_texture = klgl::Texture::from_bytes(
                        &device,
                        &queue,
                        tutorial_content::ILLUMINATI_PNG,
                        "illuminati.png",
                    )
                    .unwrap();
                    TextureState {
                        bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &texture_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(
                                        &diffuse_texture.view,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(
                                        &diffuse_texture.sampler,
                                    ),
                                },
                            ],
                            label: Some("illuminati bind group"),
                        }),
                    }
                },
            ]
        };

        Self {
            start_time: Instant::now(),
            window: window_box,
            surface,
            device,
            queue,
            config,
            size,
            clear_color: wgpu::Color::BLACK,
            surface_configured: false,
            frame_counter: klgl::FpsCounter::new(),
            last_printed_fps: Instant::now(),
            lines_pipeline,
            lines_vertex_buffer,
            num_lines,
            models_pipeline,
            model_vertex_buffer,
            model_index_buffer,
            num_model_indices: TRIANGLE_INDICES.len() as u32,
            textures,
            active_texture: 0,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller: CameraController::new(0.2, 0.2),
        }
    }

    fn swap_model(&mut self) {
        let (vertices, indices) = {
            if self.num_model_indices == TRIANGLE_INDICES.len() as u32 {
                let mut hex_vert: [ModelVertex; 5] = HEX_VERTICES.into();
                transform_model(&mut hex_vert);
                (hex_vert.to_vec(), HEX_INDICES)
            } else {
                let mut tri_vert: [ModelVertex; 3] = TRIANGLE_VERTICES.into();
                transform_model(&mut tri_vert);
                (tri_vert.to_vec(), TRIANGLE_INDICES)
            }
        };

        self.model_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        self.model_index_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

        self.num_model_indices = indices.len() as u32;
    }

    #[allow(unused_variables)]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if self.camera_controller.process_events(&self.window, &event) {
            return;
        }

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                println!("The close button was pressed; stopping");
                event_loop.exit()
            }
            WindowEvent::Resized(physical_size) => {
                log::info!("physical_size: {physical_size:?}");
                self.surface_configured = true;
                self.resize(physical_size);
            }
            WindowEvent::RedrawRequested => {
                // This tells winit that we want another frame after this one
                self.window.request_redraw();

                if !self.surface_configured {
                    return;
                }

                self.update();
                match self.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        self.resize(self.size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other) => {
                        log::error!("OutOfMemory");
                        event_loop.exit();
                    }

                    // This happens when the a frame takes too long to present
                    Err(wgpu::SurfaceError::Timeout) => {
                        log::warn!("Surface timeout")
                    }
                }
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                self.clear_color.r = position.x as f64 / self.size.width as f64;
                self.clear_color.g = position.y as f64 / self.size.height as f64;
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                if button == MouseButton::Left && state == ElementState::Pressed {
                    self.swap_model();
                }
            }
            WindowEvent::Touch(touch) => {
                if touch.phase == TouchPhase::Started {
                    self.swap_model();
                }
            }
            _ => {}
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.camera
                .set_aspect(new_size.width as f32 / new_size.height as f32);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let since_last_print = now.duration_since(self.last_printed_fps);
        if since_last_print.as_secs_f32() > 1.0 {
            log::info!("fps: {}", self.frame_counter.framerate());
            self.last_printed_fps = now;
        }

        let dur_since_start = now.duration_since(self.start_time);
        self.active_texture =
            (((dur_since_start.as_secs_f64() / 3.0) as u32) % (self.textures.len() as u32)) as u32;

        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.frame_counter.register_entry(Instant::now());
        if !self.surface_configured {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw lines
            if self.num_lines != 0 {
                render_pass.set_pipeline(&self.lines_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.lines_vertex_buffer.slice(..));
                render_pass.draw(0..self.num_lines, 0..self.num_lines / 2);
            }

            let chosen_texture_bind_group = &self.textures[self.active_texture as usize].bind_group;
            render_pass.set_pipeline(&self.models_pipeline);
            render_pass.set_bind_group(0, chosen_texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.model_vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.model_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_model_indices, 0, 0..1);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger");
        } else {
            let env = Env::default()
                .filter_or("MY_LOG_LEVEL", "info")
                .write_style_or("MY_LOG_STYLE", "always");
            env_logger::init_from_env(env);
        }
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();

    event_loop.run_app(&mut app).unwrap();
}
