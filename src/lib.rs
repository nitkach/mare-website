mod app;
mod database;
pub mod logging;
mod utils;

pub async fn run() -> anyhow::Result<()> {
    // TODO .env file?
    // if let Err(err) = dotenvy::dotenv() {
    //     println!("Failed to load .env file: {}", err);
    // }

    app::run().await
}
