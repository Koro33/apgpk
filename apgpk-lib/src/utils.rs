use crate::error::ApgpkError;
use hex::ToHex;
use pgp::{composed::key::SecretKey, types::KeyTrait};
use std::{
    fs,
    io::{self, BufRead},
    path::Path,
};

pub fn key2hex(k: &SecretKey) -> String {
    k.fingerprint().encode_hex_upper::<String>()
}

pub fn save_key(k: &SecretKey, dir: impl AsRef<Path>) -> Result<String, ApgpkError> {
    let armored_key = k.to_owned().sign(String::new)?.to_armored_string(None)?;

    let fp = k.fingerprint().encode_hex_upper::<String>();
    let filename = format!("{}.asc", &fp);
    let path = dir.as_ref().join(filename);

    std::fs::write(path, armored_key)?;
    Ok(fp)
}

pub fn check_output_dir<T>(path: T) -> Result<(), ApgpkError>
where
    T: AsRef<Path>,
{
    let path = path.as_ref();
    if path.exists() {
        if path.is_file() {
            return Err(ApgpkError::Other(format!(
                "Path `{}` is a file, not a directory",
                path.display()
            )));
        }
    } else {
        log::warn!("Path `{}` doesn't exist, creating...", path.display());
        fs::create_dir(path)?;
    }
    Ok(())
}

pub fn parse_pattern<T>(path: T) -> Result<Vec<String>, ApgpkError>
where
    T: AsRef<Path>,
{
    let mut pattern = vec![];

    if !path.as_ref().exists() {
        return Err(ApgpkError::Other(format!(
            "Pattern file `{}` doesn't exists",
            path.as_ref().display()
        )));
    }

    if path.as_ref().is_dir() {
        return Err(ApgpkError::Other(format!(
            "Path `{}` isn't a file, cannot parse patterns from it",
            path.as_ref().display()
        )));
    }

    let f = fs::File::open(path.as_ref())?;
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
        log::warn!("Too short(<=4) patterns are included, this may cause perfermance issue. For secure those patterns are ignored")
    }

    if pattern.is_empty() {
        let default_pattern = "ABCDEF".to_string();
        log::warn!(
            "Warning: No pattern found, use default pattern `{}`",
            default_pattern
        );
        pattern.push(default_pattern);
    }

    Ok(pattern)
}
