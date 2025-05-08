fn main() {
    println!(
        "cargo::rustc-env=SMUDGY_BUILD_NAME={}-{}-{}",
        std::env::var("PROFILE").unwrap(),
        std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap()
    );
}
