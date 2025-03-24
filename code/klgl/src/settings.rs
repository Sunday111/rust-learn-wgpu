cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub const DEPLOYMENT_SUB_PATH: &'static str = "rust-learn-wgpu";
    } else {
        pub const DEPLOYMENT_SUB_PATH: &'static str = "";
    }
}
