use pollster::FutureExt;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::models_draw_pass::ModelsDrawPass;
use crate::{display_depth_draw_pass::DisplayDepthDrawPass, lines_draw_pass::LinesDrawPass};
use klgl::{Camera, CameraController, CameraUniform, Rotator};

use cgmath::Deg;
use std::{iter, pin::Pin};
use web_time::Instant;

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
    last_stat_print: Instant,

    depth_texture: klgl::Texture,
    lines_draw_pass: LinesDrawPass,
    models_draw_pass: ModelsDrawPass,
    display_depth_draw_pass: Option<DisplayDepthDrawPass>,

    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,

    show_depth: bool,
}

pub struct App<'a> {
    renderer: Option<Renderer<'a>>,
}

impl<'a> App<'a> {
    pub fn new() -> Self {
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

impl<'a> Renderer<'a> {
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

        let depth_texture =
            klgl::Texture::create_depth_texture(&device, size.width, size.height, "depth_texture");

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
            (5.41923, 0.19568399, 6.468395).into(),
            // have it look at the origin
            Rotator {
                yaw: Deg(81.0),
                pitch: Deg(56.0),
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

        let depth_stencil_state = Some(wgpu::DepthStencilState {
            format: klgl::Texture::DEPTH_FORMAT,
            depth_write_enabled: true,
            // The depth_compare function tells us when to discard a new pixel.
            // Using LESS means pixels will be drawn front to back.
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let models_draw_pass = ModelsDrawPass::new(
            &device,
            &queue,
            &camera_bind_group_layout,
            config.format,
            depth_stencil_state.clone(),
        );

        let lines_draw_pass = LinesDrawPass::new(
            &device,
            &camera_bind_group_layout,
            config.format,
            depth_stencil_state,
        );

        Self {
            start_time: Instant::now(),
            window: window_box,
            surface,
            device,
            queue,
            config,
            size,
            depth_texture,
            clear_color: wgpu::Color::BLACK,
            surface_configured: false,
            frame_counter: klgl::FpsCounter::new(),
            last_stat_print: Instant::now(),
            lines_draw_pass,
            models_draw_pass,
            display_depth_draw_pass: None,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller: CameraController::new(0.2, 0.2),
            show_depth: false,
        }
    }

    #[allow(unused_variables)]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if self.camera_controller.process_events(&event) {
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
            WindowEvent::KeyboardInput { event, .. } => match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    println!("The close button was pressed; stopping");
                    event_loop.exit()
                }
                PhysicalKey::Code(KeyCode::KeyO) => {
                    self.show_depth = event.state == ElementState::Pressed;
                }
                _ => {}
            },
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
                    self.models_draw_pass.swap_model(&self.device);
                }
            }
            WindowEvent::Touch(touch) => {
                if touch.phase == TouchPhase::Started {
                    self.models_draw_pass.swap_model(&self.device);
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
            self.depth_texture = klgl::Texture::create_depth_texture(
                &self.device,
                self.config.width,
                self.config.height,
                "depth_texture",
            );

            match &mut self.display_depth_draw_pass {
                Some(draw_pass) => draw_pass.on_resize(&self.device, &self.depth_texture),
                _ => {}
            }

            self.camera
                .set_aspect(new_size.width as f32 / new_size.height as f32);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let since_last_print = now.duration_since(self.last_stat_print);
        if since_last_print.as_secs_f32() > 5.0 {
            self.last_stat_print = now;
            log::info!("fps: {}", self.frame_counter.framerate());
            log::info!(
                "eye: {:?}, rotator: {:?}",
                self.camera.get_eye(),
                self.camera.get_rotator()
            );
        }

        let dur_since_start = now.duration_since(self.start_time);
        self.models_draw_pass.set_active_texture(
            (((dur_since_start.as_secs_f64() / 3.0) as u32)
                % (self.models_draw_pass.textures.len() as u32)) as u32,
        );

        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        self.models_draw_pass.update_model_instances(
            &self.queue,
            Deg(90.0 + 80.0 * (dur_since_start.as_secs_f32() * 2.0).sin()),
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
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.lines_draw_pass
                .render(&mut render_pass, &self.camera_bind_group);

            self.models_draw_pass
                .render(&mut render_pass, &self.camera_bind_group);
        }

        if self.show_depth {
            if self.display_depth_draw_pass.is_none() {
                self.display_depth_draw_pass = Some(DisplayDepthDrawPass::new(
                    &self.device,
                    self.config.format,
                    &self.depth_texture,
                ));
            }

            match &mut self.display_depth_draw_pass {
                Some(draw_pass) => {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Display Depth Render Pass"),
                        color_attachments: &[
                            // This is what @location(0) in the fragment shader targets
                            Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                        ],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    draw_pass.render(&mut render_pass);
                }
                _ => {}
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
