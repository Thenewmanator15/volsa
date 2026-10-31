//! The plugin helper as a program of its own, for this crate's tests: what `voxa.exe
//! --plugin-helper` does, with no window and nothing else.

use std::process::ExitCode;

fn main() -> ExitCode {
    voxa_plugins::helper::serve_stdio()
}
