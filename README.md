# apgpk

[![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/Koro33/apgpk?include_prereleases)](https://github.com/Koro33/apgpk/releases) [![GitHub repo size](https://img.shields.io/github/repo-size/Koro33/apgpk)](https://github.com/Koro33/apgpk/archive/main.zip) [![Rust](https://img.shields.io/badge/Rust-stable-brightgreen)](https://www.rust-lang.org/) [![license](https://img.shields.io/github/license/Koro33/apgpk)](https://github.com/Koro33/apgpk/blob/main/LICENSE) [![GitHub Repo stars](https://img.shields.io/github/stars/Koro33/apgpk?style=social)](https://github.com/Koro33/apgpk)

A PGP key with fingerprint `FFFF FFFF` is much better than one with `86C6 F0AE` (at least looks like

Use this tool to find an awesome PGP key, whose fingerprint matches a specific pattern. Currently it only supports ECC key and suffix matching.

## Usage

```sh
$ ./apgpk --help
Find an awesome PGP key

Usage: apgpk [OPTIONS] --pattern <PATH>

Options:
  -p, --pattern <PATH>
          Path of the pattern file, one pattern per line
      --output <PATH>
          Output directory to save the key [default: ./key]
      --threads <THREADS>
          Numbers of threads to calculate [default: 8]
      --max-backshift <MAX_BACKSHIFT>
          The max backshift of time when calculating keys [default: 86400]
      --uid <UID>
          Default uid [default: apgpker]
  -h, --help
          Print help information (use `--help` for more detail)
  -V, --version
          Print version information
```

The path of the pattern file must be given by `-p` or `--pattern`. It can contain multiple patterns, one pattern per line. For example:

```txt
AAAAAAAA
ABCDEF0
EE2EE2EE
0123456789ABCDEF
FFFFFF
```

> The patterns with length less than 4 are not recommended, which may result in too many keys being generated.

```log
$ ./apgpk -p pattern -o ./key
2022-10-11T22:55:08.712217Z  INFO apgpk: Runing with 8 threads
2022-10-11T22:55:08.712235Z  INFO apgpk: Find key by pattern ["AAAAAAAA", "ABCDEF0", "EE2EE2EE", "0123456789ABCDEF", "FFFFFF"]
2022-10-11T22:55:38.751304Z  INFO apgpk: Current speed (8 threads) 186166.36 key/s
...
2022-10-11T22:57:09.989945Z  INFO apgpk: Current speed (8 threads) 187553.42 key/s
2022-10-11T22:57:18.375451Z  INFO apgpk: Find key: 65611DC454F49F3851422E3B97694D5749FFFFFF
...
2022-10-11T22:58:40.756333Z  INFO apgpk: Current speed (8 threads) 186955.45 key/s
...
# Press Ctrl+C or Send SIGNINT to kill
2022-10-11T22:58:42.915362Z  WARN apgpk: SIGNINT received, waiting all threads to exit...
2022-10-11T22:58:44.870096Z  INFO apgpk: Shutdown
```

You can find the keys in output directory, which match the pattern. Choose an awesome one and use `gpg --import {FINGERPRINT}.asc` to import it. Then you can edit the key, change the default uid or set passphrase for it.

## Compile

```sh
git clone https://github.com/Koro33/apgpk.git
cd apgpk
cargo run --release
```

## License

This project is licensed under the [AGPL-3.0](https://github.com/Koro33/apgpk/blob/main/LICENSE) License
