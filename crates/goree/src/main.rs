use goreecloud_containers_core::ContainerId;
use goreecloud_containers_image::registry::{RegistryClient, RegistryReference};
use goreecloud_containers_image::rootfs::RootfsPolicy;
use goreecloud_containers_image::{ContentStore, DEFAULT_MAX_CONTENT_BYTES, Sha256Digest};
use goreecloud_containers_oci::{OciConfig, initialize_linux_bundle};
use goreecloud_containers_runtime::{
    OciRuntimeKind, ProcessOciRuntime, RuntimeConfig, RuntimeExecution, RuntimeExecutor,
};
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
        Some("bundle") => run_bundle(args),
        Some("image") => run_image(args),
        Some("runtime") => run_runtime(args),
        Some("container") => run_container(args),
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command '{command}'\n\n{}", help_text())),
    }
}

fn run_bundle(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("init") => {
            let bundle = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute bundle path".to_owned())?,
            );
            let process_args: Vec<String> = args.collect();
            let config =
                OciConfig::minimal_linux(process_args).map_err(|error| error.to_string())?;
            let initialized =
                initialize_linux_bundle(&bundle, &config).map_err(|error| error.to_string())?;
            println!("bundle: {}", initialized.bundle_path.display());
            println!("rootfs: {}", initialized.rootfs_path.display());
            println!("config: {}", initialized.config_path.display());
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown bundle command '{command}'\n\n{}",
            help_text()
        )),
        None => Err(format!("missing bundle command\n\n{}", help_text())),
    }
}

fn run_image(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("ingest") => {
            let digest = args
                .next()
                .ok_or_else(|| "missing expected sha256 digest".to_owned())?
                .parse::<Sha256Digest>()
                .map_err(|error| error.to_string())?;
            let source = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute source-file path".to_owned())?,
            );
            let store_root = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute content-store root".to_owned())?,
            );
            let max_content_bytes = match args.next() {
                Some(value) => value
                    .parse::<u64>()
                    .map_err(|_| "max-content-bytes must be an unsigned integer".to_owned())?,
                None => DEFAULT_MAX_CONTENT_BYTES,
            };
            ensure_no_extra_args(args)?;

            let store = ContentStore::open(store_root, max_content_bytes)
                .map_err(|error| error.to_string())?;
            let content = store
                .ingest_file(digest, &source)
                .map_err(|error| error.to_string())?;
            println!("digest: {}", content.digest);
            println!("size: {}", content.size);
            println!("content: {}", content.path.display());
            println!("reused-existing: {}", content.reused_existing);
            Ok(())
        }
        Some("pull") => {
            let registry_base_url = args
                .next()
                .ok_or_else(|| "missing registry base URL".to_owned())?;
            let repository = args
                .next()
                .ok_or_else(|| "missing registry repository".to_owned())?;
            let image_reference = args
                .next()
                .ok_or_else(|| "missing image tag or digest reference".to_owned())?;
            let store_root = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute content-store root".to_owned())?,
            );
            let rootfs_target = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute rootfs target".to_owned())?,
            );
            ensure_no_extra_args(args)?;

            let reference = RegistryReference::parse(
                &registry_base_url,
                repository,
                image_reference,
            )
            .map_err(|error| error.to_string())?;
            let store = ContentStore::open(store_root, DEFAULT_MAX_CONTENT_BYTES)
                .map_err(|error| error.to_string())?;
            let pulled = RegistryClient::new()
                .pull_image(
                    &reference,
                    &store,
                    &rootfs_target,
                    RootfsPolicy::default(),
                )
                .map_err(|error| error.to_string())?;

            println!("manifest: {}", pulled.manifest_digest);
            println!("config: {}", pulled.config_digest);
            println!("layers: {}", pulled.layers.len());
            println!("os: {}", pulled.configuration.os);
            println!("architecture: {}", pulled.configuration.architecture);
            println!("rootfs: {}", pulled.rootfs.rootfs_path.display());
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown image command '{command}'\n\n{}",
            help_text()
        )),
        None => Err(format!("missing image command\n\n{}", help_text())),
    }
}

fn run_runtime(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("probe") => {
            let kind = parse_runtime_kind(args.next())?;
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
        Some("create") => {
            let (runtime, id) = parse_runtime_execution_target(&mut args)?;
            let bundle = PathBuf::from(
                args.next()
                    .ok_or_else(|| "missing absolute OCI bundle path".to_owned())?,
            );
            ensure_no_extra_args(args)?;
            let execution = runtime
                .create(&RuntimeExecutor::default(), &id, &bundle)
                .map_err(|error| error.to_string())?;
            emit_execution_output(&execution);
            Ok(())
        }
        Some("start") => {
            let (runtime, id) = parse_runtime_execution_target(&mut args)?;
            ensure_no_extra_args(args)?;
            let execution = runtime
                .start(&RuntimeExecutor::default(), &id)
                .map_err(|error| error.to_string())?;
            emit_execution_output(&execution);
            Ok(())
        }
        Some("state") => {
            let (runtime, id) = parse_runtime_execution_target(&mut args)?;
            ensure_no_extra_args(args)?;
            let execution = runtime
                .state(&RuntimeExecutor::default(), &id)
                .map_err(|error| error.to_string())?;
            emit_execution_output(&execution);
            Ok(())
        }
        Some("delete") => {
            let (runtime, id) = parse_runtime_execution_target(&mut args)?;
            ensure_no_extra_args(args)?;
            let execution = runtime
                .delete(&RuntimeExecutor::default(), &id)
                .map_err(|error| error.to_string())?;
            emit_execution_output(&execution);
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown runtime command '{command}'\n\n{}",
            help_text()
        )),
        None => Err(format!("missing runtime command\n\n{}", help_text())),
    }
}

fn parse_runtime_execution_target(
    args: &mut impl Iterator<Item = String>,
) -> Result<(ProcessOciRuntime, ContainerId), String> {
    let kind = parse_runtime_kind_required(args.next())?;
    let executable = PathBuf::from(
        args.next()
            .ok_or_else(|| "missing absolute OCI runtime executable path".to_owned())?,
    );
    let id = ContainerId::parse(
        args.next()
            .ok_or_else(|| "missing container identifier".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    let runtime = ProcessOciRuntime::new_for_execution(kind, executable)
        .map_err(|error| error.to_string())?;
    Ok((runtime, id))
}

fn parse_runtime_kind_required(raw: Option<String>) -> Result<OciRuntimeKind, String> {
    let value = raw.ok_or_else(|| "missing OCI runtime kind; expected crun or runc".to_owned())?;
    value
        .parse::<OciRuntimeKind>()
        .map_err(|error| error.to_string())
}

fn parse_runtime_kind(raw: Option<String>) -> Result<OciRuntimeKind, String> {
    match raw {
        Some(value) => value
            .parse::<OciRuntimeKind>()
            .map_err(|error| error.to_string()),
        None => Ok(OciRuntimeKind::Crun),
    }
}

fn emit_execution_output(execution: &RuntimeExecution) {
    let stdout = execution.stdout.text_lossy();
    if !stdout.is_empty() {
        print!("{stdout}");
        if execution.stdout.truncated {
            eprintln!("\nwarning: OCI runtime stdout was truncated");
        }
    }

    let stderr = execution.stderr.text_lossy();
    if !stderr.is_empty() {
        eprint!("{stderr}");
        if execution.stderr.truncated {
            eprintln!("\nwarning: OCI runtime stderr was truncated");
        }
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
        Some(command) => Err(format!(
            "unknown container command '{command}'\n\n{}",
            help_text()
        )),
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
    "GoreeCloud Containers Development CLI\n\nUsage:\n  goree version\n  goree bundle init <absolute-bundle> <command> [args...]\n  goree image ingest <sha256:digest> <absolute-source-file> <absolute-store-root> [max-content-bytes]\n  goree image pull <registry-base-url> <repository> <tag-or-sha256-digest> <absolute-store-root> <absolute-new-rootfs>\n  goree runtime probe [crun|runc] [executable]\n  goree runtime create <crun|runc> <absolute-executable> <container-id> <absolute-bundle>\n  goree runtime start <crun|runc> <absolute-executable> <container-id>\n  goree runtime state <crun|runc> <absolute-executable> <container-id>\n  goree runtime delete <crun|runc> <absolute-executable> <container-id>\n  goree container validate-id <container-id>\n\nImage retrieval, rootfs construction, and lifecycle execution are Development-only. Registry credential authentication, image-index selection, symbolic-link/hard-link layer entries, real registry/runtime acceptance, rootless, and production acceptance remain pending."
}
