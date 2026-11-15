//! Development commands for the AEI workspace. Run with `cargo xtask <command>`.

mod bundle;
mod check;
mod install;
mod package;
mod ui_test;

use std::path::PathBuf;
use std::process::ExitCode;

type Result<T = ()> = std::result::Result<T, String>;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check                      validate pins and bundle contents, then fmt, clippy and tests
  package [--out DIR]        write a reproducible dist/aei-<version>.hnmod and its .sha256
  ui-test SOCKET --out DIR   drive a running disposable game through the recipe viewer
  install [--home DIR] [--instance NAME] [--launcher PATH]
                             package the tree into an instance and native-build it
                             (defaults: ~/.local/share/horizon, horizon, horizon-launcher)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") if args.len() == 1 => check::run(),
        Some("package") => match &args[1..] {
            [] => package::run(&bundle::root().join("dist")),
            [key, out] if key == "--out" => package::run(&PathBuf::from(out)),
            _ => Err(USAGE.into()),
        },
        Some("ui-test") => match &args[1..] {
            [socket, key, out] if key == "--out" => {
                ui_test::run(&PathBuf::from(socket), &PathBuf::from(out))
            }
            _ => Err(USAGE.into()),
        },
        Some("install") => install_args(&args[1..]),
        Some("help" | "--help" | "-h") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn install_args(args: &[String]) -> Result {
    let mut home = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/share/horizon"))
        .ok_or("HOME is not set")?;
    let mut instance = "horizon".to_owned();
    let mut launcher = "horizon-launcher".to_owned();
    for pair in args.chunks(2) {
        match pair {
            [key, value] if key == "--home" => home = PathBuf::from(value),
            [key, value] if key == "--instance" => instance = value.clone(),
            [key, value] if key == "--launcher" => launcher = value.clone(),
            _ => return Err(USAGE.into()),
        }
    }
    install::run(&home, &instance, &launcher)
}
