use winresource::WindowsResource;

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("cupertino".to_string());

    slint_build::compile_with_config("ui/main_window.slint", config)
    .unwrap();

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("assets/icon256.ico")
            .compile()
            .unwrap();
    }
}
