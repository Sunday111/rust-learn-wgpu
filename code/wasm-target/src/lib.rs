#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

macro_rules! wasm_bindgen_async_fn {
    ($fn_name:ident, $target:ident) => {
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
        pub async fn $fn_name() {
            ($target::run()).await;
        }
    };
}

wasm_bindgen_async_fn!(run_tutorial_2, tutorial2_surface);
wasm_bindgen_async_fn!(run_tutorial_3, tutorial3_pipeline);
wasm_bindgen_async_fn!(run_tutorial_4, tutorial4_buffers_and_indices);
wasm_bindgen_async_fn!(run_tutorial_5, tutorial5_textures);
wasm_bindgen_async_fn!(run_tutorial_6, tutorial6_uniforms);
wasm_bindgen_async_fn!(run_tutorial_7, tutorial7_instancing);
wasm_bindgen_async_fn!(run_tutorial_8, tutorial8_depth);
wasm_bindgen_async_fn!(run_tutorial_9, tutorial9_model_loading);
