#[cfg(unix)]
use std::io;
use std::{
    process::exit,
    sync::{OnceLock, atomic::Ordering},
    thread::{self, ThreadId},
};

use ruthenium_config::{LoadConfig, RutheniumConfig};
use ruthenium_core::{RutheniumServer, SERVER_EXIT_CODE, stop_or_exit_server};
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::Instant;
use tracing::{error, info, warn};

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

#[tokio::main]
async fn main() {
    let _ = MAIN_THREAD.set(thread::current().id());

    let _ = rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("Rayon-Worker-{i}"))
        .build_global();

    let _time = Instant::now();

    let exec_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let config = RutheniumConfig::load(&exec_dir);

    ruthenium_core::init_logger(&config.advanced);

    info!("Starting...");
    tokio::spawn(async {
        if let Err(err) = setup_sighandler().await {
            eprintln!("Unable to setup signal handlers: {err}");
            error!("Unable to setup signal handlers: {err}");
        }
    });

    let ruthenium_server = RutheniumServer::new(config.basic, config.advanced)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to start server {e}");
            exit(1)
        });

    info!("Server started.");

    ruthenium_server.start().await;

    info!("Server stopped...");
    exit(SERVER_EXIT_CODE.load(Ordering::Acquire));
}

fn handle_interrupt() {
    warn!("Receieved interrupt signal; stopping server...");
    stop_or_exit_server();
}

#[cfg(unix)]
async fn setup_sighandler() -> io::Result<()> {
    let mut interupt = signal(SignalKind::interrupt())?;
    let mut hangup = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;

    let received = tokio::select! {
        received = interupt.recv() => received,
        received = hangup.recv() => received,
        received = terminate.recv() => received,
    };

    if received.is_some() {
        handle_interrupt();
    };

    Ok(())
}
