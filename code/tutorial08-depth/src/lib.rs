#[cfg(not(target_arch = "wasm32"))]
use env_logger::Env;

use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod display_depth_draw_pass;
mod lines_draw_pass;
mod models_draw_pass;

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

    let mut app = crate::app::App::new();
    event_loop.run_app(&mut app).unwrap();
}
