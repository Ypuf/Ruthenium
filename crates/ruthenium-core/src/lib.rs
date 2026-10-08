#[cfg(target_family = "unix")]
use std::time::Duration;
use std::{
    io::{ErrorKind, IsTerminal, stdin},
    process::exit,
    str::FromStr,
    sync::{
        Arc, LazyLock, OnceLock,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
};

use rustyline::{Config, Editor, error::ReadlineError, history::FileHistory};
use ruthenium_config::{AdvancedConfig, BasicConfig};
#[cfg(target_family = "unix")]
use tokio::time::sleep;
use tokio::{net::TcpListener, select};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error, info, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::{
    logging::{ConsoleWriter, ReadlineLogWrapper, RutheniumCommand},
    net::{PacketHandlerResult, java::pending::PendingConnection},
    server::Server,
};

pub mod logging;
pub mod net;
pub mod server;

pub struct LoggingConfig {
    pub color: bool,
    pub threads: bool,
    pub thread_ids: bool,
    pub target: bool,
    pub timestamp: bool,
}

pub type LoggerOption = Option<(ReadlineLogWrapper, LevelFilter, LoggingConfig)>;
pub static LOGGER_IMPL: LazyLock<Arc<OnceLock<LoggerOption>>> =
    LazyLock::new(|| Arc::new(OnceLock::new()));

pub fn init_logger(advanced: &AdvancedConfig) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;

    let logger = advanced.logging.enabled.then(|| {
        let level = std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .or(Some(advanced.logging.level.as_str()))
            .map(LevelFilter::from_str)
            .and_then(Result::ok)
            .unwrap_or(LevelFilter::INFO);

        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            let level_str = match level {
                LevelFilter::OFF => "off",
                LevelFilter::ERROR => "error",
                LevelFilter::WARN => "warn",
                LevelFilter::INFO => "info",
                LevelFilter::DEBUG => "debug",
                LevelFilter::TRACE => "trace",
            };
            EnvFilter::new(level_str)
        });

        let (logger, rl): (ConsoleWriter, Option<Editor<RutheniumCommand, FileHistory>>) =
            if advanced.commands.use_tty && stdin().is_terminal() {
                let rl_config = Config::builder()
                    .auto_add_history(true)
                    .completion_type(rustyline::CompletionType::List)
                    .edit_mode(rustyline::EditMode::Emacs)
                    .build();
                let helper = RutheniumCommand::new();

                match Editor::with_config(rl_config) {
                    Ok(mut rl) => {
                        rl.set_helper(Some(helper));
                        let printer = rl.create_external_printer().ok().map(|p| {
                            let boxed: Box<dyn rustyline::ExternalPrinter + Send> = Box::new(p);
                            boxed
                        });
                        (ConsoleWriter::new(printer), Some(rl))
                    }
                    Err(e) => {
                        eprintln!("Failed to init console input ({e}); using simple logger");
                        (ConsoleWriter::new(None), None)
                    }
                }
            } else {
                (ConsoleWriter::new(None), None)
            };

        let fmt_layer = fmt::layer()
            .with_writer(std::sync::Mutex::new(logger))
            .with_ansi(advanced.logging.color)
            .with_ansi_sanitization(false)
            .with_target(advanced.logging.target)
            .with_thread_names(advanced.logging.threads)
            .with_thread_ids(advanced.logging.thread_id);

        if advanced.logging.timestamp {
            let local_offset =
                time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
            let format_str: &'static str =
                Box::leak(advanced.logging.timestamp_format.clone().into_boxed_str());
            let timer_format = time::format_description::parse_borrowed::<3>(format_str)
                .unwrap_or_else(|_| {
                    use time::macros::format_description;
                    format_description!(version = 3, "[hour]:[minute]:[second]")
                });
            let fmt_layer =
                fmt_layer.with_timer(fmt::time::OffsetTime::new(local_offset, timer_format));
            let registry = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer);
            registry.init();
        } else {
            let fmt_layer = fmt_layer.without_time();
            let registry = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer);
            registry.init();
        }

        let logging_config = LoggingConfig {
            color: advanced.logging.color,
            threads: advanced.logging.threads,
            thread_ids: advanced.logging.thread_id,
            target: advanced.logging.target,
            timestamp: advanced.logging.timestamp,
        };

        (ReadlineLogWrapper::new(rl), level, logging_config)
    });

    assert!(LOGGER_IMPL.set(logger).is_ok(), "Failed to set logger.")
}

pub static SHOULD_STOP: AtomicBool = AtomicBool::new(false);
pub static STOP_INTERRUPT: LazyLock<CancellationToken> = LazyLock::new(CancellationToken::new);
pub static SERVER_IS_STOPPING: AtomicBool = AtomicBool::new(false);
pub static SERVER_EXIT_CODE: AtomicI32 = AtomicI32::new(0);

pub fn stop_server() {
    SHOULD_STOP.store(true, Ordering::Relaxed);
    STOP_INTERRUPT.cancel();
}

pub fn stop_or_exit_server() {
    if SERVER_IS_STOPPING.load(Ordering::Acquire) {
        exit(SERVER_EXIT_CODE.load(Ordering::Acquire));
    }
    stop_server();
}

fn resolve_some<T: Future, D, F: FnOnce(D) -> T>(
    opt: Option<D>,
    func: F,
) -> futures::future::Either<T, std::future::Pending<T::Output>> {
    use futures::future::Either;
    opt.map_or_else(
        || Either::Right(std::future::pending()),
        |val| Either::Left(func(val)),
    )
}

pub struct RutheniumServer {
    pub server: Arc<Server>,
    pub tcp_listener: Option<TcpListener>,
}

impl RutheniumServer {
    pub async fn new(
        basic_config: BasicConfig,
        advanced_config: AdvancedConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let server = Server::new(basic_config, advanced_config).await?;

        let tcp_listener = if server.advanced_config.networking.java.enabled {
            let address = server.advanced_config.networking.java.address;

            let listener = match TcpListener::bind(address).await {
                Ok(l) => l,
                Err(e) => match e.kind() {
                    ErrorKind::AddrInUse => {
                        error!("Error: Address {address} is already in use.");
                        error!("Make sure another instance of the server isn't already running");
                        std::process::exit(1);
                    }
                    ErrorKind::PermissionDenied => {
                        error!("Error: Permission denied when binding to {address}.");
                        error!("You might need sudo/admin privileges to use ports below 1024");
                        std::process::exit(1);
                    }
                    ErrorKind::AddrNotAvailable => {
                        error!("Error: The address {address} is not available on this machine");
                        std::process::exit(1);
                    }
                    _ => {
                        error!("Failed to start TcpListener on {address}: {e}");
                        std::process::exit(1);
                    }
                },
            };

            let _addr = listener.local_addr().unwrap_or_else(|_| {
                std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0)
            });

            Some(listener)
        } else {
            None
        };

        Ok(Self {
            server,
            tcp_listener,
        })
    }

    pub async fn start(&self) {
        if self.server.advanced_config.commands.use_console
            && let Some((wrapper, _, _)) = LOGGER_IMPL.wait()
        {
            if let Some(rl) = wrapper.take_readline() {
                setup_console(rl, self.server.clone());
            } else {
                if self.server.advanced_config.commands.use_tty {
                    warn!("Input is not TTY; using simple logger");
                }
                setup_stdin_console(&self.server);
            }
        }

        let tasks = Arc::new(TaskTracker::new());
        let mut master_client_id: u64 = 0;

        while !SHOULD_STOP.load(Ordering::Relaxed) {
            if !self.listener_task(&mut master_client_id, &tasks).await {
                break;
            }
        }

        SERVER_IS_STOPPING.store(true, Ordering::Release);

        info!("Stopped accepting connections");

        info!("Ending tasks");

        tasks.close();
        tasks.wait().await;

        if let Some((wrapper, _, _)) = LOGGER_IMPL.wait()
            && let Some(rl) = wrapper.take_readline()
        {
            let _ = rl;
        }
    }

    pub async fn listener_task(
        &self,
        master_client_id_counter: &mut u64,
        tasks: &Arc<TaskTracker>,
    ) -> bool {
        select! {
            tcp_result = resolve_some(self.tcp_listener.as_ref(), tokio::net::TcpListener::accept) => {
                match tcp_result {
                    Ok((connection, client_addr)) => {
                        if let Err(e) = connection.set_nodelay(true) {
                            error!("Failed to set nodelay: {e}");
                        }

                        let client_id = *master_client_id_counter;
                        *master_client_id_counter += 1;

                        let formatted_address = format!("{client_addr}");
                        debug!("Accepted connection from {formatted_address} (id {client_id})");
                        let server_clone = self.server.clone();

                        tasks.spawn(async move {
                            let mut pending = PendingConnection::new(
                                connection,
                                client_addr,
                                client_id,
                                Arc::downgrade(&server_clone),
                            );
                            let login_result = pending.handle_login_sequence(&server_clone).await;

                            match login_result {
                                PacketHandlerResult::Stop => {
                                    pending.close();
                                },
                                PacketHandlerResult::ReadyToPlay(_profile, _config) => {
                                    todo!("")
                                },
                            }
                        });
                    }
                    Err(e) => {
                        #[cfg(target_family = "unix")]
                        if e.raw_os_error() == Some(libc::EMFILE) {
                            error!("Too many open files");
                            sleep(Duration::from_millis(500)).await;
                            return true;
                        }
                        error!("Failed to accept connection: {e}");
                        sleep(Duration::from_millis(50)).await;
                    }
                }
            },

            () = STOP_INTERRUPT.cancelled() => {
                return false;
            }
        }
        true
    }
}

fn setup_stdin_console(server: &Arc<Server>) {
    let (tx, mut _rx) = tokio::sync::mpsc::channel(1);
    let rt = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let mut line = String::new();
            if let Ok(size) = stdin().read_line(&mut line) {
                if size == 0 {
                    break;
                }
            } else {
                break;
            }
            if line.is_empty() || line.as_bytes()[line.len() - 1] != b'\n' {
                warn!("Console input was not terminated with a newline");
            }
            let _ = rt.block_on(tx.send(line.trim().to_string()));
        }
    });
    let _server_clone = server.clone();
    // command handling here...
}

fn setup_console(mut rl: Editor<RutheniumCommand, FileHistory>, server: Arc<Server>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let (tx_reply, mut rx_reply) = tokio::sync::mpsc::channel::<i32>(1);

    if let Some(helper) = rl.helper_mut() {
        if let Ok(mut server_lock) = helper.server.write() {
            *server_lock = Some(server.clone());
        }
        let _ = helper.rt.set(tokio::runtime::Handle::current());
    }

    std::thread::spawn(move || {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let readline = rl.readline("> ");
            match readline {
                Ok(line) => {
                    let _ = rl.add_history_entry(line.clone());
                    if tx.blocking_send(line).is_err() {
                        break;
                    }

                    let _ = rx_reply.blocking_recv();
                }
                Err(ReadlineError::Interrupted) => {
                    info!("CTRL-C");
                    stop_or_exit_server();
                    break;
                }
                Err(ReadlineError::Eof) => {
                    info!("CTRL-D");
                    stop_server();
                    break;
                }
                Err(err) => {
                    error!("Error reading console input: {err}");
                    break;
                }
            }
        }
        if let Some((wrapper, _, _)) = LOGGER_IMPL.wait() {
            wrapper.return_readline(rl);
        }
    });

    server.clone().spawn_task(async move {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let t1 = rx.recv();
            let t2 = STOP_INTERRUPT.cancelled();

            let result = select! {
                line = t1 => line,
                () = t2 => None
            };

            if let Some(_line) = result {
                // server command handling here...

                let _ = tx_reply.send(1).await;
            } else {
                break;
            }
        }
        drop(rx);
        debug!("Stopped console task");
    });
}
