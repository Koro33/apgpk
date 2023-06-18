use crate::error::ApgpkError;
use chrono::prelude::*;
use hex::ToHex;
use pgp::{
    composed::{
        key::{SecretKey, SecretKeyParamsBuilder},
        KeyType,
    },
    types::KeyTrait,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::Instant,
};

pub fn task(
    uid: String,
    max_backshift: i64,
    pars: &[String],
    exit_signal: &Arc<AtomicBool>,
    msg_tx: &Sender<Msg>,
) -> Result<(), ApgpkError> {
    let loop_begin = Instant::now();
    let t = Utc::now();
    let time_block = 60 * 60;

    let mut pgp_builder = SecretKeyParamsBuilder::default();
    pgp_builder
        .key_type(KeyType::EdDSA)
        .can_create_certificates(true)
        .can_sign(true)
        .primary_user_id(uid)
        .created_at(t);

    for start in 0..=(max_backshift / time_block) {
        for backshift in start * time_block..(start * time_block + time_block) {
            pgp_builder.created_at(t - chrono::Duration::seconds(backshift));
            let k = pgp_builder.build().unwrap().generate().unwrap(); // can't fail
            let k_fp = k.fingerprint().encode_hex_upper::<String>();
            for par in pars {
                if k_fp.ends_with(par) {
                    msg_tx.send(Msg::Key(Box::new(k.clone())))?;
                }
            }
        }
        if exit_signal.load(Ordering::Relaxed) {
            // drop(msg_tx);
            break;
        }
        msg_tx.send(Msg::Speed(
            (start * time_block + time_block) as f64
                / (loop_begin.elapsed().as_millis() as f64 / 1000.),
        ))?;
    }

    Ok(())
}



#[derive(Debug)]
pub enum Msg {
    Key(Box<SecretKey>),
    Speed(f64),
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread;

    #[test]
    fn test_fs() {
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();
        let tx = msg_tx.clone();
        let handler = thread::spawn(move || -> Result<(), ApgpkError> {
            task(
                "test".to_string(),
                60 * 60 * 24,
                &["FFFFFF".to_string()],
                &Arc::new(AtomicBool::new(false)),
                &tx,
            )
            .unwrap();
            Ok(())
        });
        drop(msg_tx);
        for msg in msg_rx {
            match msg {
                Msg::Key(k) => {
                    println!("key: {}", k.fingerprint().encode_hex_upper::<String>());
                }
                Msg::Speed(speed) => {
                    println!("speed: {}", speed);
                }
            }
        }
        handler.join().unwrap().unwrap();
    }

    #[test]
    fn test_test() {
        for i in 0..=0 {
            println!("{}", i);
        }
    }
}
