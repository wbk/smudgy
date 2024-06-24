use winresource::WindowsResource;

fn main() {
    let default_font = std::path::Path::new("./assets/fonts/MonaspaceKryptonVarVF.ttf");

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
    slint_build::compile_with_config("ui/main_window.slint", config).unwrap();

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("assets/icon256.ico")
            .compile()
            .unwrap();
    }

    println!(
        "cargo::rustc-env=SMUDGY_BUILD_NAME={}-{}-{}",
        std::env::var("PROFILE").unwrap(),
        std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap()
    );
}
