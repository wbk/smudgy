use tokio::net::TcpListener;

mod config;
mod game;
mod session;

use config::CONFIG;

use game::Game;
use session::Session;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Binding to {}...", CONFIG.get_bind_address());

    let listener = TcpListener::bind(CONFIG.get_bind_address())
        .await
        .expect("Could not bind to address.");

    println!("Listening...");

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("Connection in from {:?}", addr);

                    tokio::spawn(async move { Session::process(stream).await });
                }
                Err(e) => {
                    println!("Error on accept: {:?}", e)
                }
            }
        }
    });

    println!("Beginning game loop...");

    let mut game = Game::new();

    loop {
        game.pulse().await;
    }
}
