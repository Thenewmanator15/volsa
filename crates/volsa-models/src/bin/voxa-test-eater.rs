//! A program for the tests of the helper's job object only: it reads a number of MiB from its
//! input, asks for that much memory and touches every page of it, says `held` or `refused`, and
//! then waits for its input to close.

use std::io::{BufRead, Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut line = String::new();
    let stdin = std::io::stdin();
    if stdin.lock().read_line(&mut line).is_err() {
        return ExitCode::FAILURE;
    }
    let mib: usize = line.trim().parse().unwrap_or(0);
    let mut held: Vec<u8> = Vec::new();
    let answer = if held.try_reserve_exact(mib << 20).is_ok() {
        held.resize(mib << 20, 1);
        "held"
    } else {
        "refused"
    };
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{answer}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let _ = stdin.lock().read_to_end(&mut Vec::new());
    drop(held);
    ExitCode::SUCCESS
}
