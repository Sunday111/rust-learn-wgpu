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
use std::{cell::RefCell, iter, rc::Rc};
use web_time::Instant;

struct Renderer {
    file_loader: klgl::file_loader::FileLoader,
    render_context: Rc<RefCell<klgl::RenderContext>>,

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

pub struct App {
    renderer: Option<Renderer>,
}

impl App {
    pub async fn new() -> Self {
        Self { renderer: None }
    }
}

impl<'a> ApplicationHandler for App {
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

impl Renderer {
    async fn new(w: Window) -> Self {
        let render_context = Rc::new(RefCell::new(klgl::RenderContext::new(w).await));

        let size = render_context.borrow().window.inner_size();
        let depth_texture = klgl::Texture::create_depth_texture(
            &render_context.borrow().device,
            size.width,
            size.height,
            "depth_texture",
        );

        let camera_bind_group_layout = render_context.borrow().device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
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
            },
        );

        let camera = Camera::new(
            // position the camera 1 unit up and 2 units back
            // +z is out of the screen
            (19.03984, -5.1585493, 23.231775).into(),
            // have it look at the origin
            Rotator {
                yaw: Deg(159.0),
                pitch: Deg(-13.0),
                roll: Deg(0.0),
            },
            render_context.borrow().aspect(),
            90.0,
            0.1,
            1000.0,
        );

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer =
            render_context
                .borrow()
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let camera_bind_group =
            render_context
                .borrow()
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
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

        let mut file_loader = klgl::file_loader::FileLoader::new();

        let models_draw_pass = ModelsDrawPass::new(
            &mut file_loader,
            render_context.clone(),
            &camera_bind_group_layout,
            depth_stencil_state.clone(),
        )
        .await;

        let lines_draw_pass = LinesDrawPass::new(
            render_context.clone(),
            &camera_bind_group_layout,
            depth_stencil_state,
        );

        Self {
            render_context,
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
            file_loader,
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
                self.resize(physical_size.width, physical_size.height);
            }
            WindowEvent::RedrawRequested => {
                // This tells winit that we want another frame after this one
                self.render_context.borrow().window.request_redraw();

                if !self.surface_configured {
                    return;
                }

                self.update();
                match self.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let (w, h) = {
                            let ctx = self.render_context.borrow();
                            (ctx.config.width, ctx.config.height)
                        };
                        self.resize(w, h)
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
                let ctx = self.render_context.borrow();
                self.clear_color.r = position.x as f64 / ctx.config.width as f64;
                self.clear_color.g = position.y as f64 / ctx.config.height as f64;
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                if button == MouseButton::Left && state == ElementState::Pressed {
                    self.models_draw_pass.swap_model();
                }
            }
            WindowEvent::Touch(touch) => {
                if touch.phase == TouchPhase::Started {
                    self.models_draw_pass.swap_model();
                }
            }
            _ => {}
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            {
                let mut ctx = self.render_context.borrow_mut();
                ctx.resize(width, height);
                self.depth_texture = klgl::Texture::create_depth_texture(
                    &ctx.device,
                    ctx.config.width,
                    ctx.config.height,
                    "depth_texture",
                );
            }

            match &mut self.display_depth_draw_pass {
                Some(draw_pass) => {
                    let ctx = self.render_context.borrow();
                    draw_pass.on_resize(&ctx.device, &self.depth_texture)
                }
                _ => {}
            }

            let ctx = self.render_context.borrow();
            self.camera.set_aspect(ctx.aspect());
        }
    }

    fn update(&mut self) {
        self.file_loader.poll();
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

        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        self.render_context.borrow().queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        self.models_draw_pass.update();
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.frame_counter.register_entry(Instant::now());
        if !self.surface_configured {
            return Ok(());
        }

        let output = self.render_context.borrow().surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.render_context.borrow().device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            },
        );

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
                let ctx_clone = self.render_context.clone();
                let ctx = ctx_clone.borrow();
                self.display_depth_draw_pass = Some(DisplayDepthDrawPass::new(
                    &ctx.device,
                    ctx.config.format,
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

        self.render_context
            .borrow()
            .queue
            .submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
