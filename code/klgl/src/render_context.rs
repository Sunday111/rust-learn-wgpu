use std::pin::Pin;

pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub window: Pin<Box<winit::window::Window>>,
    pub surface: wgpu::Surface<'static>,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl RenderContext {
    pub async fn new(w: winit::window::Window) -> Self {
        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        // SAFETY: `boxed` is pinned, so we can safely create a reference to `window`
        let window_box = Box::pin(w);
        let window_ref: &'static winit::window::Window =
            unsafe { &*(Pin::as_ref(&window_box).get_ref() as *const _) };

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

        Self {
            instance,
            window: window_box,
            surface,
            adapter,
            device,
            queue,
        }
    }
}
