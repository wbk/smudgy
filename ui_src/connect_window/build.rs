use winresource::WindowsResource;

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("cupertino-dark".to_string());
    slint_build::compile_with_config("../../ui/connect_window/connect_window.slint", config).unwrap();
}
