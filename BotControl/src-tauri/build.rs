fn main() {
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    tauri_build::build()
}
