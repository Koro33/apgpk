use anyhow::{anyhow, bail, Context, Result};
use chrono;
use clap::Parser;
use hex::ToHex;
use pgp::{
    composed::{
        key::{SecretKey, SecretKeyParamsBuilder},
        KeyType,
    },
    types::KeyTrait,
};
use std::{
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
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
    #[arg(long, value_name = "PATH", default_value = "./key")]
    output: PathBuf,
    /// Numbers of threads to calculate
    #[arg(long, default_value_t = std::thread::available_parallelism().unwrap().get())]
    threads: usize,
    /// The max backshift of time when calculating keys.
    /// 
    /// Changing this default value is not recommended.
    #[arg(long, default_value_t = 60*60*24)]
    max_backshift: i64,
    /// Default uid
    #[arg(long, default_value_t = String::from("apgpker"))]
    uid: String,
}

fn save_key(k: &SecretKey, dir: impl AsRef<Path>) -> Result<String> {
    let armored_key = k
        .to_owned()
        .sign(String::new)?
        .to_armored_string(None)
        .map_err(|e| anyhow!(e))?;

    let fp = k.fingerprint().encode_hex_upper::<String>();
    let filename = format!("{}.asc", &fp);
    let path = dir.as_ref().join(filename);

    std::fs::write(&path, armored_key)?;
    Ok(fp)
}

fn check_output_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        if path.is_file() {
            tracing::error!("Path '{}' is not a directory", path.display());
            bail!("");
        }
    } else {
        tracing::warn!("Path '{}' doesn't exist, creating...", path.display());
        fs::create_dir(path)?;
    }
    Ok(())
}

fn parse_pattern(cli: &Cli) -> Result<Vec<String>> {
    let pattern_file = &cli.pattern;
    let mut pattern = vec![];

    if !pattern_file.exists() {
        tracing::error!("Pattern file '{}' doesn't exists", pattern_file.display());
        bail!("");
    }

    if pattern_file.is_dir() {
        tracing::error!(
            "Path '{}' isn't a file, cannot parse patterns from it",
            pattern_file.display()
        );
        bail!("");
    }

    let f = fs::File::open(pattern_file)?;
    let lines = io::BufReader::new(f).lines();
    let mut short_pattern_warning = false;
    for line in lines {
        let line = line?.trim().to_uppercase();
        match line.len() {
            0 => {}
            1..=4 => {
                short_pattern_warning = true;
            }
            _ => {
                pattern.push(line);
            }
        }
    }

    if short_pattern_warning {
        tracing::warn!("Too short(<=4) patterns are included, this may cause perfermance issue. For secure those patterns are ignored")
    }

    if pattern.is_empty() {
        let default_pattern = "ABCDEF".to_string();
        tracing::warn!(
            "Warning: No pattern found, use default pattern '{}'",
            default_pattern
        );
        pattern.push(default_pattern);
    }

    Ok(pattern)
}

fn task(cli: &Cli, pars: &Vec<String>, exit: &Arc<AtomicBool>, res_tx: &Sender<Msg>) -> Result<()> {
    let loop_begin = Instant::now();
    let t = chrono::offset::Utc::now();
    let mut backshift = 0i64;

    let mut pgp_builder = SecretKeyParamsBuilder::default();
    pgp_builder
        .key_type(KeyType::EdDSA)
        .can_create_certificates(true)
        .can_sign(true)
        .primary_user_id(cli.uid.to_owned())
        .created_at(t);

    while backshift < cli.max_backshift {
        pgp_builder.created_at(t - chrono::Duration::seconds(backshift));
        let k = pgp_builder.build().unwrap().generate().unwrap();
        let k_fp = k.fingerprint().encode_hex_upper::<String>();
        for par in pars {
            if k_fp.ends_with(par) {
                res_tx.send(Msg::Key(k.clone()))?;
            }
        }

        backshift += 1;
    }

    res_tx.send(Msg::Speed(
        cli.max_backshift as f64,
        loop_begin.elapsed().as_millis() as f64 / 1000.,
    ))?;
    Ok(())
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

enum Msg {
    Key(SecretKey),
    Speed(f64, f64), // (items, interval)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    log_init();

    let pattern = parse_pattern(&cli)?;
    tracing::info!("Find by pattern {:?}", pattern);

    check_output_dir(cli.output.to_owned())?;

    let (msg_tx, msg_rx) = mpsc::channel();
    let thread_exit = Arc::new(AtomicBool::new(false));

    // Setup ctrlc signal
    let ctrlc_exit = thread_exit.clone();
    ctrlc::set_handler(move || {
        tracing::info!("SIGNINT received, waiting all threads to exit...");
        ctrlc_exit.store(true, Ordering::Relaxed);
    })
    .with_context(|| {
        tracing::error!("Error setting Ctrl-C handler");
        anyhow!("")
    })?;

    let handles: Vec<_> = (0..cli.threads)
        .map(|i| {
            let cli = cli.clone();
            let pattern = pattern.clone();
            let res_tx = msg_tx.clone();
            let thread_exit = thread_exit.clone();

            thread::spawn(move || -> Result<()> {
                tracing::debug!("Thread {} has been created", i);
                loop {
                    task(&cli, &pattern, &thread_exit, &res_tx)?;

                    if thread_exit.load(Ordering::Relaxed) {
                        drop(res_tx);
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

    let mut current_speed = 0f64;
    let mut last_show_speed = Instant::now();
    let show_speed_interval = Duration::from_secs(30);
    for msg in msg_rx {
        match msg {
            Msg::Key(k) => {
                tracing::info!("Find key: {}", k.fingerprint().encode_hex_upper::<String>());
                save_key(&k, cli.output.to_owned())?;
            }
            Msg::Speed(items, interval) => {
                current_speed = (current_speed * interval + items) / 2. / interval;
                let now = Instant::now();
                if (now - last_show_speed) > show_speed_interval {
                    tracing::info!(
                        "Current speed ({} threads) {:.2} key/s",
                        cli.threads,
                        current_speed * cli.threads as f64
                    );
                    last_show_speed = now;
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

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_fs() {}
}
