//! aetherctl — the AetherCore headless command surface (Phase 28).
//!
//! CX-3: one truth surface. This CLI consumes the SAME `aethercore-contracts` wire
//! types and `aethercore-ipc` transports as every other client; mutations never bypass
//! service governance because every mutating verb is an RPC to the maintenance service.

mod cli;
mod envelope;
mod error;
mod exit;
mod fleet;
pub mod i18n;
mod offline;
mod render;
mod release;
mod sec;
mod service_cmds;
mod transport;
mod units;

use std::io::Write as _;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let invocation = match cli::parse(&args) {
        Ok(invocation) => invocation,
        Err(failure) => {
            // Usage failures print the typed reason + usage block on stderr and exit 2.
            eprintln!("aetherctl: {}", failure.message_key());
            if let Some(detail) = failure.usage_detail() {
                eprintln!("aetherctl: {detail}");
            }
            eprint!("{}", cli::USAGE);
            std::process::exit(exit::ExitCode::Usage.as_i32());
        }
    };
    if invocation.version_requested {
        println!("aetherctl {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(exit::ExitCode::Ok.as_i32());
    }
    let code = dispatch(invocation);
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

fn dispatch(invocation: cli::Invocation) -> i32 {
    let _active_language = invocation.lang;
    match invocation.command {
        cli::Command::Offline(job) => offline::run(&invocation.config, job),
        cli::Command::Service(job) => service_cmds::run(&invocation.config, job),
        cli::Command::Fleet(job) => fleet::run(&invocation.config, job),
    }
}
