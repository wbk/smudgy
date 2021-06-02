use tokio::net::TcpListener;

mod account;
mod config;
mod creature;
pub mod dice;
mod game;
mod session;

use game::Game;
use session::Session;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Binding to {}...", config::BIND_ADDRESS);

    let listener = TcpListener::bind(config::BIND_ADDRESS)
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
