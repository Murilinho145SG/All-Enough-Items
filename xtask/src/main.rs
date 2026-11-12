//! Development commands for the AEI workspace. Run with `cargo xtask <command>`.

mod bundle;
mod check;
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
  ui-test SOCKET --out DIR   drive a running disposable game through the recipe viewer";

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
