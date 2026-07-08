//! Command-line entry point for the A\* search simulator.

use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;
use example_project::cli::{Cli, run};

/// Parse arguments, run the simulator, and report any error to stderr.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match run(&cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(io::stderr(), "error: {err}");
            ExitCode::FAILURE
        }
    }
}
