#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Critical fix for NVIDIA + WebKitGTK 2.42+ on Linux (Wayland / XWayland / KDE)
    // Prevents the black screen bug caused by DMA-BUF / EGL buffer sharing failures
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1");
        std::env::set_var("GDK_BACKEND", "x11");
    }

    fototriagem_lib::run();
}
