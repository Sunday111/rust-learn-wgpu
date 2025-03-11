use std::{iter, pin::Pin};
use web_time::Instant;

use pollster::FutureExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod fps_counter;
use fps_counter::FpsCounter;

#[cfg(not(target_arch = "wasm32"))]
use env_logger::Env;

struct Renderer<'a> {
    window: Pin<Box<Window>>,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    clear_color: wgpu::Color,
    surface_configured: bool,
    frame_counter: FpsCounter,
    last_printed_fps: Instant,
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

        Self {
            window: window_box,
            surface: surface,
            device: device,
            queue: queue,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width,
                height: size.height,
                present_mode: surface_caps.present_modes[0],
                alpha_mode: surface_caps.alpha_modes[0],
                desired_maximum_frame_latency: 2,
                view_formats: vec![],
            },
            size: size,
            clear_color: wgpu::Color::BLACK,
            surface_configured: false,
            frame_counter: FpsCounter::new(),
            last_printed_fps: Instant::now(),
        }
    }

    #[allow(unused_variables)]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
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
            _ => {}
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config)
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let since_last_print = now.duration_since(self.last_printed_fps);
        if since_last_print.as_secs_f32() > 1.0 {
            log::info!("fps: {}", self.frame_counter.framerate());
            self.last_printed_fps = now;
        }
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
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
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
