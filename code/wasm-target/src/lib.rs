use wasm_bindgen::prelude::wasm_bindgen;

macro_rules! wasm_bindgen_async_fn {
    ($fn_name:ident, $target:ident) => {
        #[wasm_bindgen]
        pub async fn $fn_name() {
            ($target::run()).await;
        }
    };
}

wasm_bindgen_async_fn!(run_tutorial_02, tutorial02_surface);
wasm_bindgen_async_fn!(run_tutorial_03, tutorial03_pipeline);
wasm_bindgen_async_fn!(run_tutorial_04, tutorial04_buffers_and_indices);
wasm_bindgen_async_fn!(run_tutorial_05, tutorial05_textures);
wasm_bindgen_async_fn!(run_tutorial_06, tutorial06_uniforms);
wasm_bindgen_async_fn!(run_tutorial_07, tutorial07_instancing);
wasm_bindgen_async_fn!(run_tutorial_08, tutorial08_depth);
wasm_bindgen_async_fn!(run_tutorial_09, tutorial09_model_loading);
wasm_bindgen_async_fn!(run_tutorial_10, tutorial10_lights);
