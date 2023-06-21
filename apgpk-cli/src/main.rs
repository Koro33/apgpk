use anyhow::{anyhow, Context, Result};
use apgpk_lib::{core, utils};
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

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path of the pattern file, one pattern per line.
    #[arg(short, long, value_name = "PATH")]
    pattern: PathBuf,
    /// Directory to save the key
    #[arg(short, long, value_name = "PATH", default_value = "./key_output")]
    output: PathBuf,
    /// Numbers of threads to calculate, default value is the cores of cpu 
    #[arg(short, long, default_value_t = default_thread_num())]
    threads: usize,
    /// The max backshift days when calculating keys.
    ///
    /// Changing this default value is not recommended.
    #[arg(long, default_value_t = 30)]
    max_backshift_days: u16,
    /// Default uid
    #[arg(long, default_value_t = String::from("apgpk"))]
    uid: String,
}

fn default_thread_num() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

fn log_init() {
    // from env variable RUST_LOG
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::debug!("Log engine is initialized");
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    log_init();

    let pattern = utils::parse_pattern(&cli.pattern)?;
    log::info!("Runing with {} threads", cli.threads);
    log::info!("Find key by pattern {:?}", pattern);

    utils::check_output_dir(cli.output.clone())?;

    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<core::Msg>();
    let thread_exit = Arc::new(AtomicBool::new(false));

    let exit = thread_exit.clone();

    // Setup ctrlc signal
    ctrlc::set_handler(move || {
        log::warn!("SIGNINT received, waiting all threads to exit...");
        exit.store(true, Ordering::Relaxed);
    })
    .with_context(|| {
        log::error!("Error setting Ctrl-C handler");
        anyhow!("")
    })?;

    let handles: Vec<_> = (0..cli.threads)
        .map(|i| {
            let cli = cli.clone();
            let pattern = pattern.clone();
            let tx = msg_tx.clone();
            let thread_exit = thread_exit.clone();

            thread::spawn(move || -> Result<()> {
                log::debug!("Thread {} has been created", i);
                loop {
                    core::task(
                        cli.uid.clone(),
                        cli.max_backshift_days,
                        &pattern,
                        &thread_exit,
                        &tx,
                    )?;

                    if thread_exit.load(Ordering::Relaxed) {
                        drop(tx);
                        break;
                    }
                }
                log::debug!("Thread {} complete", i);
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
            core::Msg::Key(k) => {
                log::info!("Find key: {}", utils::key2hex(&k));
                utils::save_key(&k, cli.output.clone())?;
            }
            core::Msg::Speed(current_speed) => {
                let now = Instant::now();
                avrg_speed = (2.0 * avrg_speed + current_speed) / 3.0;
                if (now - last_show) > show_speed_interval {
                    log::info!(
                        "Current speed estimated ({} threads) {:.2} key/s",
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

    log::info!("Shutdown");

    Ok(())
}
