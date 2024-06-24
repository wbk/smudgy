fn main() {
    let default_font = std::path::Path::new("../../assets/fonts/MonaspaceKryptonVarVF.ttf");

    if !default_font.is_file() {
        panic!("Could not load default font");
    }

    println!(
        "cargo::rustc-env=SLINT_DEFAULT_FONT={}",
        default_font
            .canonicalize()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap()
    );

    let config = slint_build::CompilerConfiguration::new().with_style("cupertino-dark".to_string());
    slint_build::compile_with_config("../../ui/connect_window/connect_window.slint", config).unwrap();
}
