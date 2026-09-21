//! Embeds the application icon and a manifest declaring per-monitor DPI awareness.

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon.ico");
    // No manifest here on purpose: winit already embeds one, and a second
    // non-default manifest makes the resource merge fail. DPI awareness comes
    // from winit, which sets it programmatically at startup.
    resource.set("ProductName", "Volume11");
    resource.set("FileDescription", "Volume11");

    if let Err(error) = resource.compile() {
        println!("cargo:warning=resource compilation failed: {error}");
    }
}
