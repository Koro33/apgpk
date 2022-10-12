use anyhow::{anyhow, Context, Result};
use chrono::{offset::Utc, Duration};
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
};

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value_t = std::thread::available_parallelism().unwrap().get())]
    threads: usize,
    #[arg(long, default_value_t = 60*60*24*30)]
    max_backshift: i64,
    #[arg(long, default_value_t = String::from("apgpker"))]
    uid: String,
    #[arg(long, default_value = "./pattern")]
    pattern: PathBuf,
    #[arg(long, default_value = "./key")]
    output: PathBuf,
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
            return Err(anyhow!("path is not dir"));
        }
    } else {
        println!(
            "Warning: Path '{}' is not exist, creating...",
            path.display()
        );
        fs::create_dir(path)?;
    }
    Ok(())
}

fn parse_pattern(cli: &Cli) -> Result<Vec<String>> {
    let pattern_file = &cli.pattern;
    let mut pattern = vec![];

    if pattern_file.exists() {
        if !pattern_file.is_file() {
            return Err(anyhow!(
                "Path {} isn't a file, cannot parse patterns from it",
                pattern_file.display()
            ));
        }
    } else {
        fs::File::create(pattern_file).with_context(|| anyhow!("Cannot create pattern file"))?;
    }

    let f = fs::File::open(pattern_file)?;
    let lines = io::BufReader::new(f).lines();
    let mut warning_displayed = false;
    for line in lines {
        let line = line?.trim().to_uppercase();
        match line.len() {
            0 => {}
            1..=4 => {
                if !warning_displayed {
                    println!("Warning: too short(<=4) patterns are included, this may cause perfermance issue.");
                    println!("Warning: For secure those patterns are ignored");
                    warning_displayed = true;
                }
            }
            _ => {
                pattern.push(line);
            }
        }
    }
    if pattern.is_empty() {
        let default_pattern = "ABCDEF".to_string();
        println!(
            "Warning: No patterns found, use default pattern '{}'",
            default_pattern
        );
        pattern.push(default_pattern);
    }
    Ok(pattern)
}

fn task(
    cli: &Cli,
    pars: &Vec<String>,
    exit: &Arc<AtomicBool>,
    res_tx: &Sender<SecretKey>,
) -> Result<()> {
    let t = Utc::now();
    let mut backshift = 0i64;

    let mut pgp_builder = SecretKeyParamsBuilder::default();
    pgp_builder
        .key_type(KeyType::EdDSA)
        .can_create_certificates(true)
        .can_sign(true)
        .primary_user_id(cli.uid.to_owned())
        .created_at(t);

    while backshift < cli.max_backshift {
        pgp_builder.created_at(t - Duration::seconds(backshift));
        let k = pgp_builder.build().unwrap().generate().unwrap();
        let k_fp = k.fingerprint().encode_hex_upper::<String>();
        for par in pars {
            if k_fp.ends_with(par) {
                res_tx.send(k.clone())?;
            }
        }

        if backshift % (60 * 60) == 0 {
            if exit.load(Ordering::Relaxed) {
                drop(res_tx);
                break;
            }
        }

        backshift += 1;
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let pattern = parse_pattern(&cli)?;
    println!("Given Pattern {:?}", pattern);
    check_output_dir(cli.output.to_owned())?;

    let (res_tx, res_rx) = mpsc::channel();
    let thread_exit = Arc::new(AtomicBool::new(false));

    // Setup ctrlc signal
    let ctrlc_exit = thread_exit.clone();
    ctrlc::set_handler(move || {
        ctrlc_exit.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let mut thread_pool = vec![];
    for i in 0..cli.threads {
        let cli = cli.clone();
        let pattern = pattern.clone();
        let res_tx = res_tx.clone();
        let thread_exit = thread_exit.clone();

        let handle = thread::spawn(move || -> Result<()> {
            println!("Thread {} has been created", i);
            loop {
                task(&cli, &pattern, &thread_exit, &res_tx)?;

                if thread_exit.load(Ordering::Relaxed) {
                    drop(res_tx);
                    break;
                }
            }
            println!("Thread {} complete", i);
            Ok(())
        });
        thread_pool.push(handle);
    }

    // drop original tx
    drop(res_tx);

    for k in res_rx {
        println!("Find key: {}", k.fingerprint().encode_hex_upper::<String>());
        save_key(&k, cli.output.to_owned())?;
    }

    println!("SIGNINT received, exit...");

    for handle in thread_pool {
        handle.join().unwrap().unwrap();
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_mpsc() {}
}
