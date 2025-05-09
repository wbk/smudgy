use winresource::WindowsResource;

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("../assets/icon256.ico")
            .compile()
            .unwrap();
    }
}
