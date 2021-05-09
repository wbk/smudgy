use std::error::Error;

use smudgy::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Smudgy {} starting...", env!("CARGO_PKG_VERSION"));
    run().await.unwrap();

    Ok(())
}
