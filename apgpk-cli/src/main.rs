use anyhow::{anyhow, Context, Result};
use apgpk_lib::{unit, utils};
use clap::Parser;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tracing_subscriber::{prelude::*, EnvFilter};

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path of the pattern file, one pattern per line.
    #[arg(short, long, value_name = "PATH")]
    pattern: PathBuf,
    /// Output directory to save the key
    #[arg(short, long, value_name = "PATH", default_value = "./key_output")]
    output: PathBuf,
    /// Numbers of threads to calculate
    #[arg(short, long, default_value_t = default_thread_num())]
    threads: usize,
    /// The max backshift of time when calculating keys.
    ///
    /// Changing this default value is not recommended.
    #[arg(long, default_value_t = 60*60*24*30)]
    max_backshift: i64,
    /// Default uid
    #[arg(long, default_value_t = String::from("apgpk"))]
    uid: String,
}

fn default_thread_num() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

fn log_init() {
    // from env variable RUST_LOG
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("apgpk=info"));
    let formatting_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);
    tracing_subscriber::registry()
        .with(env_filter)
        .with(formatting_layer)
        .init();
    tracing::debug!("Log engine is initialized");
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    log_init();

    let pattern = utils::parse_pattern(&cli.pattern)?;
    tracing::info!("Runing with {} threads", cli.threads);
    tracing::info!("Find key by pattern {:?}", pattern);

    utils::check_output_dir(cli.output.clone())?;

    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<unit::Msg>();
    let thread_exit = Arc::new(AtomicBool::new(false));

    let exit = thread_exit.clone();

    // Setup ctrlc signal
    ctrlc::set_handler(move || {
        tracing::warn!("SIGNINT received, waiting all threads to exit...");
        exit.store(true, Ordering::Relaxed);
    })
    .with_context(|| {
        tracing::error!("Error setting Ctrl-C handler");
        anyhow!("")
    })?;

    let handles: Vec<_> = (0..cli.threads)
        .map(|i| {
            let cli = cli.clone();
            let pattern = pattern.clone();
            let tx = msg_tx.clone();
            let thread_exit = thread_exit.clone();

            thread::spawn(move || -> Result<()> {
                tracing::debug!("Thread {} has been created", i);
                loop {
                    unit::task(
                        cli.uid.clone(),
                        cli.max_backshift,
                        &pattern,
                        &thread_exit,
                        &tx,
                    )?;

                    if thread_exit.load(Ordering::Relaxed) {
                        drop(tx);
                        break;
                    }
                }
                tracing::debug!("Thread {} complete", i);
                Ok(())
            })
        })
        .collect();

    // drop original tx
    drop(msg_tx);

    let mut last_show = Instant::now();
    let mut avrg_speed = 0.0;
    let show_speed_interval = Duration::from_secs(15);
    for msg in msg_rx {
        match msg {
            unit::Msg::Key(k) => {
                tracing::info!(
                    "Find key: {}",
                    utils::key2hex(&k)
                );
                utils::save_key(&k, cli.output.clone())?;
            }
            unit::Msg::Speed(current_speed) => {
                let now = Instant::now();
                avrg_speed = (2.0 * avrg_speed + current_speed) / 3.0;
                if (now - last_show) > show_speed_interval {
                    tracing::info!(
                        "Current speed ({} threads) {:.2} key/s",
                        cli.threads,
                        avrg_speed * cli.threads as f64
                    );
                    last_show = now;
                }
            }
        }
    }

    handles.into_iter().for_each(|h| {
        h.join().unwrap().unwrap();
    });

    tracing::info!("Shutdown");

    Ok(())
}

