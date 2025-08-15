use std::pin::Pin;

pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub window: Pin<Box<winit::window::Window>>,
    pub surface: wgpu::Surface<'static>,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl RenderContext {
    pub async fn test_backends(backends: wgpu::Backends) -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: None,
                ..Default::default()
            })
            .await;

        adapter.is_ok()
    }

    pub async fn create_any(w: winit::window::Window) -> Self {
        // Successfully created RenderContext takes window object
        if Self::test_backends(wgpu::Backends::PRIMARY).await {
            Self::new(w, wgpu::Backends::PRIMARY).await.unwrap()
        } else {
            Self::new(w, wgpu::Backends::SECONDARY).await.unwrap()
        }
    }

    pub async fn new(w: winit::window::Window, backends: wgpu::Backends) -> anyhow::Result<Self> {
        // The instance is a handle to our GPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        // SAFETY: `boxed` is pinned, so we can safely create a reference to `window`
        let window_box = Box::pin(w);
        let window: &'static winit::window::Window =
            unsafe { &*(Pin::as_ref(&window_box).get_ref() as *const _) };

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

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
        log::info!("surface format: {:?}", surface_format);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
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
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|err| anyhow::anyhow!("Failed to request device. Error: {:?}", err))?;

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
                    let canvas = web_sys::Element::from(window.canvas()?);
                    dst.append_child(&canvas).ok()?;
                    Some(())
                })
                .expect("Couldn't append canvas to document body.");
        }

        let size = window.inner_size();
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

        Ok(Self {
            instance,
            window: window_box,
            surface,
            adapter,
            device,
            queue,
            config,
        })
    }

    pub fn aspect(&self) -> f32 {
        return self.config.width as f32 / self.config.height as f32;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
