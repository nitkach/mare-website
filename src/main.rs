use futures::{FutureExt, TryFutureExt};
use std::{panic::AssertUnwindSafe, process::ExitCode};
use tracing::error;

#[tokio::main]
async fn main() -> ExitCode {
    let log_control = mare_website::logging::LogControl::init_logging();

    let website = AssertUnwindSafe(async {
        let result = mare_website::run().await;

        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        }
    })
    .catch_unwind()
    .unwrap_or_else(|_| {
        error!("Exiting due to a panic...");
        ExitCode::FAILURE
    });

    let exit_code = website.await;

    // let filter = EnvFilter::from_default_env().add_directive("sqlx::query=off".parse()?);

    // // Initialize the logger with the specified configuration
    // tracing_subscriber::fmt::Subscriber::builder()
    //     .with_env_filter(filter)
    //     .init();

    log_control.shutdown().await;

    exit_code
}
