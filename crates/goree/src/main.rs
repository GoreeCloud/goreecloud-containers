use goreecloud_containers_core::ContainerId;
use goreecloud_containers_runtime::{OciRuntimeKind, ProcessOciRuntime, RuntimeConfig};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("version") => {
            ensure_no_extra_args(args)?;
            println!("goree {VERSION} (GoreeCloud Containers Development)");
            Ok(())
        }
        Some("runtime") => run_runtime(args),
        Some("container") => run_container(args),
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command '{command}'\n\n{}", help_text())),
    }
}

fn run_runtime(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("probe") => {
            let kind = match args.next() {
                Some(value) => value.parse::<OciRuntimeKind>().map_err(|error| error.to_string())?,
                None => OciRuntimeKind::Crun,
            };
            let executable = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(kind.as_str()));
            ensure_no_extra_args(args)?;

            let runtime = ProcessOciRuntime::new(RuntimeConfig { kind, executable });
            let probe = runtime.probe().map_err(|error| error.to_string())?;
            println!("runtime: {}", probe.kind);
            println!("executable: {}", probe.executable.display());
            println!("version: {}", probe.version_output);
            Ok(())
        }
        Some(command) => Err(format!("unknown runtime command '{command}'\n\n{}", help_text())),
        None => Err(format!("missing runtime command\n\n{}", help_text())),
    }
}

fn run_container(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("validate-id") => {
            let raw = args
                .next()
                .ok_or_else(|| "missing container identifier".to_owned())?;
            ensure_no_extra_args(args)?;
            let id = ContainerId::parse(raw).map_err(|error| error.to_string())?;
            println!("valid container id: {id}");
            Ok(())
        }
        Some(command) => Err(format!("unknown container command '{command}'\n\n{}", help_text())),
        None => Err(format!("missing container command\n\n{}", help_text())),
    }
}

fn ensure_no_extra_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument '{extra}'"));
    }
    Ok(())
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    "GoreeCloud Containers Development CLI\n\nUsage:\n  goree version\n  goree runtime probe [crun|runc] [executable]\n  goree container validate-id <container-id>\n\nThis Development CLI does not execute containers yet."
}
