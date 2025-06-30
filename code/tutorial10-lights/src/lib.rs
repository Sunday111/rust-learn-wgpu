#[cfg(not(target_arch = "wasm32"))]
use env_logger::Env;

use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod display_depth_draw_pass;
mod lines_draw_pass;
mod model;
mod models_draw_pass;

pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(|panic_info| {
                    let message = format!("💥 Panic occurred!\n\n{}", panic_info);

                    #[cfg(target_arch = "wasm32")]
                    {
                        web_sys::window()
                            .and_then(|win| win.document())
                            .and_then(|doc| {
                                if let Some(body) = doc.body() {
                                    // Clear the body content
                                    body.set_inner_html("");

                                    // Create a <pre> element to show the panic nicely
                                    let pre = doc.create_element("pre").unwrap();
                                    pre.set_text_content(Some(&message));
                                    pre.set_attribute(
                                        "style",
                                        "white-space: pre-wrap; color: red; font-family: monospace; padding: 1em;",
                                    )
                                    .unwrap();

                                    body.append_child(&pre).unwrap();
                                }
                                Some(())
                            });
                    }

                    log::error!("{}", message);
            }));
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

    let mut app = crate::app::App::new().await;
    event_loop.run_app(&mut app).unwrap();
}
