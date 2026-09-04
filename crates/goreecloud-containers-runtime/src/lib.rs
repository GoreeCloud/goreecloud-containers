use goreecloud_containers_core::ContainerId;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const OCI_CONFIG_FILENAME: &str = "config.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OciRuntimeKind {
    Crun,
    Runc,
}

impl OciRuntimeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crun => "crun",
            Self::Runc => "runc",
        }
    }
}

impl fmt::Display for OciRuntimeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for OciRuntimeKind {
    type Err = RuntimeKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "crun" => Ok(Self::Crun),
            "runc" => Ok(Self::Runc),
            _ => Err(RuntimeKindError(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeKindError(String);

impl fmt::Display for RuntimeKindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported OCI runtime '{}'; expected 'crun' or 'runc'",
            self.0
        )
    }
}

impl Error for RuntimeKindError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub kind: OciRuntimeKind,
    pub executable: PathBuf,
}

impl RuntimeConfig {
    #[must_use]
    pub fn for_kind(kind: OciRuntimeKind) -> Self {
        Self {
            kind,
            executable: PathBuf::from(kind.as_str()),
        }
    }

    pub fn validated_for_execution(
        kind: OciRuntimeKind,
        executable: impl Into<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(RuntimeError::ExecutablePathNotAbsolute(executable));
        }

        let canonical = fs::canonicalize(&executable).map_err(|source| RuntimeError::Io {
            operation: "canonicalize OCI runtime executable",
            path: executable.clone(),
            source,
        })?;
        let metadata = fs::metadata(&canonical).map_err(|source| RuntimeError::Io {
            operation: "inspect OCI runtime executable",
            path: canonical.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(RuntimeError::ExecutableNotRegularFile(canonical));
        }
        if !is_executable(&metadata) {
            return Err(RuntimeError::ExecutableNotExecutable(canonical));
        }

        Ok(Self {
            kind,
            executable: canonical,
        })
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl RuntimeCommand {
    #[must_use]
    pub fn display_lossy(&self) -> String {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(self.program.to_string_lossy().into_owned());
        parts.extend(
            self.args
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned()),
        );
        parts.join(" ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProbe {
    pub kind: OciRuntimeKind,
    pub executable: PathBuf,
    pub version_output: String,
}

#[derive(Debug)]
pub enum RuntimeError {
    Spawn {
        executable: PathBuf,
        source: io::Error,
    },
    NonZeroExit {
        executable: PathBuf,
        code: Option<i32>,
        stderr: String,
    },
    EmptyVersionOutput {
        executable: PathBuf,
    },
    ExecutablePathNotAbsolute(PathBuf),
    ExecutableNotRegularFile(PathBuf),
    ExecutableNotExecutable(PathBuf),
    BundlePathNotAbsolute(PathBuf),
    BundlePathSymlink(PathBuf),
    BundleNotDirectory(PathBuf),
    BundleConfigSymlink(PathBuf),
    BundleConfigNotRegularFile(PathBuf),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { executable, source } => write!(
                formatter,
                "failed to execute OCI runtime '{}': {source}",
                executable.display()
            ),
            Self::NonZeroExit {
                executable,
                code,
                stderr,
            } => write!(
                formatter,
                "OCI runtime '{}' exited with status {:?}: {}",
                executable.display(),
                code,
                stderr.trim()
            ),
            Self::EmptyVersionOutput { executable } => write!(
                formatter,
                "OCI runtime '{}' returned no version output",
                executable.display()
            ),
            Self::ExecutablePathNotAbsolute(path) => write!(
                formatter,
                "OCI runtime executable path must be absolute for lifecycle execution: {}",
                path.display()
            ),
            Self::ExecutableNotRegularFile(path) => write!(
                formatter,
                "OCI runtime executable is not a regular file: {}",
                path.display()
            ),
            Self::ExecutableNotExecutable(path) => write!(
                formatter,
                "OCI runtime executable does not have an executable permission bit: {}",
                path.display()
            ),
            Self::BundlePathNotAbsolute(path) => write!(
                formatter,
                "OCI bundle path must be absolute to avoid working-directory ambiguity: {}",
                path.display()
            ),
            Self::BundlePathSymlink(path) => write!(
                formatter,
                "OCI bundle path must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::BundleNotDirectory(path) => {
                write!(
                    formatter,
                    "OCI bundle path is not a directory: {}",
                    path.display()
                )
            }
            Self::BundleConfigSymlink(path) => write!(
                formatter,
                "OCI bundle config.json must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::BundleConfigNotRegularFile(path) => write!(
                formatter,
                "OCI bundle config.json is not a regular file: {}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} '{}': {source}",
                path.display()
            ),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

impl CapturedOutput {
    #[must_use]
    pub fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExecution {
    pub code: Option<i32>,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeExecutionPolicy {
    pub timeout: Duration,
    pub max_output_bytes_per_stream: usize,
    pub poll_interval: Duration,
}

impl Default for RuntimeExecutionPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_output_bytes_per_stream: 1024 * 1024,
            poll_interval: Duration::from_millis(10),
        }
    }
}

#[derive(Debug)]
pub enum RuntimeExecutionError {
    Spawn {
        program: PathBuf,
        source: io::Error,
    },
    Wait {
        program: PathBuf,
        source: io::Error,
    },
    Kill {
        program: PathBuf,
        source: io::Error,
    },
    Read {
        stream: &'static str,
        source: io::Error,
    },
    ReaderPanicked {
        stream: &'static str,
    },
    TimedOut {
        program: PathBuf,
        timeout: Duration,
        stdout: CapturedOutput,
        stderr: CapturedOutput,
    },
    NonZeroExit {
        program: PathBuf,
        code: Option<i32>,
        stdout: CapturedOutput,
        stderr: CapturedOutput,
    },
}

impl fmt::Display for RuntimeExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, source } => {
                write!(
                    formatter,
                    "failed to spawn '{}': {source}",
                    program.display()
                )
            }
            Self::Wait { program, source } => {
                write!(
                    formatter,
                    "failed while waiting for '{}': {source}",
                    program.display()
                )
            }
            Self::Kill { program, source } => {
                write!(
                    formatter,
                    "failed to terminate '{}': {source}",
                    program.display()
                )
            }
            Self::Read { stream, source } => {
                write!(formatter, "failed while draining child {stream}: {source}")
            }
            Self::ReaderPanicked { stream } => {
                write!(formatter, "child {stream} reader thread panicked")
            }
            Self::TimedOut {
                program,
                timeout,
                stderr,
                ..
            } => write!(
                formatter,
                "'{}' exceeded execution timeout of {} ms: {}",
                program.display(),
                timeout.as_millis(),
                stderr.text_lossy().trim()
            ),
            Self::NonZeroExit {
                program,
                code,
                stderr,
                ..
            } => write!(
                formatter,
                "'{}' exited with status {:?}: {}",
                program.display(),
                code,
                stderr.text_lossy().trim()
            ),
        }
    }
}

impl Error for RuntimeExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source, .. }
            | Self::Wait { source, .. }
            | Self::Kill { source, .. }
            | Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeExecutor {
    policy: RuntimeExecutionPolicy,
}

impl RuntimeExecutor {
    #[must_use]
    pub const fn new(policy: RuntimeExecutionPolicy) -> Self {
        Self { policy }
    }

    #[must_use]
    pub const fn policy(&self) -> RuntimeExecutionPolicy {
        self.policy
    }

    fn execute(&self, command: &RuntimeCommand) -> Result<RuntimeExecution, RuntimeExecutionError> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| RuntimeExecutionError::Spawn {
                program: command.program.clone(),
                source,
            })?;

        let stdout = child
            .stdout
            .take()
            .expect("piped stdout must be available after successful spawn");
        let stderr = child
            .stderr
            .take()
            .expect("piped stderr must be available after successful spawn");
        let max_output_bytes = self.policy.max_output_bytes_per_stream;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, max_output_bytes));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, max_output_bytes));

        let started = Instant::now();
        let status = loop {
            match child
                .try_wait()
                .map_err(|source| RuntimeExecutionError::Wait {
                    program: command.program.clone(),
                    source,
                })? {
                Some(status) => break status,
                None if started.elapsed() >= self.policy.timeout => {
                    if let Err(source) = child.kill() {
                        if source.kind() != io::ErrorKind::InvalidInput {
                            return Err(RuntimeExecutionError::Kill {
                                program: command.program.clone(),
                                source,
                            });
                        }
                    }
                    child.wait().map_err(|source| RuntimeExecutionError::Wait {
                        program: command.program.clone(),
                        source,
                    })?;

                    let (stdout, stderr) = finish_capture(stdout_reader, stderr_reader)?;
                    return Err(RuntimeExecutionError::TimedOut {
                        program: command.program.clone(),
                        timeout: self.policy.timeout,
                        stdout,
                        stderr,
                    });
                }
                None => thread::sleep(self.policy.poll_interval),
            }
        };

        let (stdout, stderr) = finish_capture(stdout_reader, stderr_reader)?;
        let execution = RuntimeExecution {
            code: status.code(),
            stdout,
            stderr,
        };
        if status.success() {
            Ok(execution)
        } else {
            Err(RuntimeExecutionError::NonZeroExit {
                program: command.program.clone(),
                code: execution.code,
                stdout: execution.stdout,
                stderr: execution.stderr,
            })
        }
    }
}

impl Default for RuntimeExecutor {
    fn default() -> Self {
        Self::new(RuntimeExecutionPolicy::default())
    }
}

fn finish_capture(
    stdout_reader: JoinHandle<io::Result<CapturedOutput>>,
    stderr_reader: JoinHandle<io::Result<CapturedOutput>>,
) -> Result<(CapturedOutput, CapturedOutput), RuntimeExecutionError> {
    let stdout_result = join_capture(stdout_reader, "stdout");
    let stderr_result = join_capture(stderr_reader, "stderr");
    Ok((stdout_result?, stderr_result?))
}

fn join_capture(
    reader: JoinHandle<io::Result<CapturedOutput>>,
    stream: &'static str,
) -> Result<CapturedOutput, RuntimeExecutionError> {
    match reader.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(RuntimeExecutionError::Read { stream, source }),
        Err(_) => Err(RuntimeExecutionError::ReaderPanicked { stream }),
    }
}

fn read_bounded(mut reader: impl Read, max_output_bytes: usize) -> io::Result<CapturedOutput> {
    let mut captured = Vec::with_capacity(max_output_bytes.min(16 * 1024));
    let mut truncated = false;
    let mut chunk = [0_u8; 8192];

    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }

        let remaining = max_output_bytes.saturating_sub(captured.len());
        let keep = remaining.min(count);
        captured.extend_from_slice(&chunk[..keep]);
        if keep != count {
            truncated = true;
        }
    }

    Ok(CapturedOutput {
        bytes: captured,
        truncated,
    })
}

#[derive(Clone, Debug)]
pub struct ProcessOciRuntime {
    config: RuntimeConfig,
}

impl ProcessOciRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }

    pub fn new_for_execution(
        kind: OciRuntimeKind,
        executable: impl Into<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::new(RuntimeConfig::validated_for_execution(
            kind, executable,
        )?))
    }

    #[must_use]
    pub const fn kind(&self) -> OciRuntimeKind {
        self.config.kind
    }

    pub fn probe(&self) -> Result<RuntimeProbe, RuntimeError> {
        let output = Command::new(&self.config.executable)
            .arg("--version")
            .output()
            .map_err(|source| RuntimeError::Spawn {
                executable: self.config.executable.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(RuntimeError::NonZeroExit {
                executable: self.config.executable.clone(),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let version_output = if stdout.is_empty() { stderr } else { stdout };
        if version_output.is_empty() {
            return Err(RuntimeError::EmptyVersionOutput {
                executable: self.config.executable.clone(),
            });
        }

        Ok(RuntimeProbe {
            kind: self.config.kind,
            executable: self.config.executable.clone(),
            version_output,
        })
    }

    pub fn plan_create(
        &self,
        id: &ContainerId,
        bundle_path: &Path,
    ) -> Result<RuntimeCommand, RuntimeError> {
        if !bundle_path.is_absolute() {
            return Err(RuntimeError::BundlePathNotAbsolute(
                bundle_path.to_path_buf(),
            ));
        }
        Ok(RuntimeCommand {
            program: self.config.executable.clone(),
            args: vec![
                OsString::from("create"),
                OsString::from("--bundle"),
                bundle_path.as_os_str().to_owned(),
                OsString::from(id.as_str()),
            ],
        })
    }

    #[must_use]
    pub fn plan_start(&self, id: &ContainerId) -> RuntimeCommand {
        RuntimeCommand {
            program: self.config.executable.clone(),
            args: vec![OsString::from("start"), OsString::from(id.as_str())],
        }
    }

    #[must_use]
    pub fn plan_state(&self, id: &ContainerId) -> RuntimeCommand {
        RuntimeCommand {
            program: self.config.executable.clone(),
            args: vec![OsString::from("state"), OsString::from(id.as_str())],
        }
    }

    #[must_use]
    pub fn plan_delete(&self, id: &ContainerId) -> RuntimeCommand {
        RuntimeCommand {
            program: self.config.executable.clone(),
            args: vec![OsString::from("delete"), OsString::from(id.as_str())],
        }
    }

    pub fn create(
        &self,
        executor: &RuntimeExecutor,
        id: &ContainerId,
        bundle_path: &Path,
    ) -> Result<RuntimeExecution, LifecycleExecutionError> {
        let bundle_path = validate_bundle(bundle_path)?;
        let command = self.plan_create(id, &bundle_path)?;
        Ok(executor.execute(&command)?)
    }

    pub fn start(
        &self,
        executor: &RuntimeExecutor,
        id: &ContainerId,
    ) -> Result<RuntimeExecution, LifecycleExecutionError> {
        Ok(executor.execute(&self.plan_start(id))?)
    }

    pub fn state(
        &self,
        executor: &RuntimeExecutor,
        id: &ContainerId,
    ) -> Result<RuntimeExecution, LifecycleExecutionError> {
        Ok(executor.execute(&self.plan_state(id))?)
    }

    pub fn delete(
        &self,
        executor: &RuntimeExecutor,
        id: &ContainerId,
    ) -> Result<RuntimeExecution, LifecycleExecutionError> {
        Ok(executor.execute(&self.plan_delete(id))?)
    }
}

#[derive(Debug)]
pub enum LifecycleExecutionError {
    Validation(RuntimeError),
    Execution(RuntimeExecutionError),
}

impl fmt::Display for LifecycleExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::Execution(error) => error.fmt(formatter),
        }
    }
}

impl Error for LifecycleExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::Execution(error) => Some(error),
        }
    }
}

impl From<RuntimeError> for LifecycleExecutionError {
    fn from(error: RuntimeError) -> Self {
        Self::Validation(error)
    }
}

impl From<RuntimeExecutionError> for LifecycleExecutionError {
    fn from(error: RuntimeExecutionError) -> Self {
        Self::Execution(error)
    }
}

fn validate_bundle(bundle_path: &Path) -> Result<PathBuf, RuntimeError> {
    if !bundle_path.is_absolute() {
        return Err(RuntimeError::BundlePathNotAbsolute(
            bundle_path.to_path_buf(),
        ));
    }

    let metadata = fs::symlink_metadata(bundle_path).map_err(|source| RuntimeError::Io {
        operation: "inspect OCI bundle",
        path: bundle_path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RuntimeError::BundlePathSymlink(bundle_path.to_path_buf()));
    }
    if !metadata.is_dir() {
        return Err(RuntimeError::BundleNotDirectory(bundle_path.to_path_buf()));
    }

    let canonical_bundle = fs::canonicalize(bundle_path).map_err(|source| RuntimeError::Io {
        operation: "canonicalize OCI bundle",
        path: bundle_path.to_path_buf(),
        source,
    })?;
    let config_path = canonical_bundle.join(OCI_CONFIG_FILENAME);
    let config_metadata =
        fs::symlink_metadata(&config_path).map_err(|source| RuntimeError::Io {
            operation: "inspect OCI bundle config.json",
            path: config_path.clone(),
            source,
        })?;
    if config_metadata.file_type().is_symlink() {
        return Err(RuntimeError::BundleConfigSymlink(config_path));
    }
    if !config_metadata.is_file() {
        return Err(RuntimeError::BundleConfigNotRegularFile(config_path));
    }

    Ok(canonical_bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn id() -> ContainerId {
        match ContainerId::parse("example") {
            Ok(id) => id,
            Err(error) => panic!("test identifier should be valid: {error}"),
        }
    }

    fn temporary_directory(test_name: &str) -> PathBuf {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "goreecloud-containers-runtime-{test_name}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    fn fake_runtime(test_name: &str, script_body: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = temporary_directory(test_name);
        assert!(fs::create_dir_all(&directory).is_ok());
        let script = directory.join("fake-runtime");
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)
        {
            Ok(file) => file,
            Err(error) => panic!("failed to create fake runtime: {error}"),
        };
        assert!(file.write_all(script_body.as_bytes()).is_ok());
        let mut permissions = match file.metadata() {
            Ok(metadata) => metadata.permissions(),
            Err(error) => panic!("failed to inspect fake runtime: {error}"),
        };
        permissions.set_mode(0o755);
        assert!(fs::set_permissions(&script, permissions).is_ok());
        (directory, script)
    }

    #[test]
    fn parses_supported_runtime_kinds() {
        assert_eq!("crun".parse::<OciRuntimeKind>(), Ok(OciRuntimeKind::Crun));
        assert_eq!("runc".parse::<OciRuntimeKind>(), Ok(OciRuntimeKind::Runc));
        assert!("docker".parse::<OciRuntimeKind>().is_err());
    }

    #[test]
    fn plans_create_command_with_absolute_bundle() {
        let runtime = ProcessOciRuntime::new(RuntimeConfig::for_kind(OciRuntimeKind::Crun));
        let command = runtime.plan_create(&id(), Path::new("/var/lib/goreecloud/bundles/example"));
        let command = match command {
            Ok(command) => command,
            Err(error) => panic!("absolute bundle should be accepted: {error}"),
        };
        assert_eq!(
            command.display_lossy(),
            "crun create --bundle /var/lib/goreecloud/bundles/example example"
        );
    }

    #[test]
    fn rejects_relative_bundle_path() {
        let runtime = ProcessOciRuntime::new(RuntimeConfig::for_kind(OciRuntimeKind::Runc));
        assert!(matches!(
            runtime.plan_create(&id(), Path::new("relative/bundle")),
            Err(RuntimeError::BundlePathNotAbsolute(_))
        ));
    }

    #[test]
    fn plans_lifecycle_commands_without_executing_them() {
        let runtime = ProcessOciRuntime::new(RuntimeConfig::for_kind(OciRuntimeKind::Runc));
        assert_eq!(
            runtime.plan_start(&id()).display_lossy(),
            "runc start example"
        );
        assert_eq!(
            runtime.plan_state(&id()).display_lossy(),
            "runc state example"
        );
        assert_eq!(
            runtime.plan_delete(&id()).display_lossy(),
            "runc delete example"
        );
    }

    #[cfg(unix)]
    #[test]
    fn executes_lifecycle_through_a_bounded_fake_runtime() {
        let script_body = r#"#!/bin/sh
case "$1" in
  create) printf 'created\n' ;;
  start) printf 'started\n' ;;
  state) printf '{"ociVersion":"1.3.0","id":"example","status":"running"}\n' ;;
  delete) printf 'deleted\n' ;;
  *) printf 'unsupported\n' >&2; exit 64 ;;
esac
"#;
        let (directory, script) = fake_runtime("lifecycle", script_body);
        let bundle = directory.join("bundle");
        assert!(fs::create_dir_all(&bundle).is_ok());
        assert!(fs::write(bundle.join(OCI_CONFIG_FILENAME), "{}\n").is_ok());

        let runtime = match ProcessOciRuntime::new_for_execution(OciRuntimeKind::Crun, script) {
            Ok(runtime) => runtime,
            Err(error) => panic!("fake runtime should validate: {error}"),
        };
        let executor = RuntimeExecutor::default();

        let create = runtime.create(&executor, &id(), &bundle);
        assert!(create.is_ok());
        let start = runtime.start(&executor, &id());
        assert!(start.is_ok());
        let state = match runtime.state(&executor, &id()) {
            Ok(state) => state,
            Err(error) => panic!("fake state should succeed: {error}"),
        };
        assert!(
            state
                .stdout
                .text_lossy()
                .contains("\"status\":\"running\"")
        );
        let delete = runtime.delete(&executor, &id());
        assert!(delete.is_ok());

        assert!(fs::remove_dir_all(directory).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn bounds_captured_output() {
        let command = RuntimeCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from("printf '1234567890'")],
        };
        let executor = RuntimeExecutor::new(RuntimeExecutionPolicy {
            max_output_bytes_per_stream: 4,
            ..RuntimeExecutionPolicy::default()
        });
        let execution = match executor.execute(&command) {
            Ok(execution) => execution,
            Err(error) => panic!("bounded command should succeed: {error}"),
        };

        assert_eq!(execution.stdout.bytes, b"1234");
        assert!(execution.stdout.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn terminates_command_after_timeout() {
        let command = RuntimeCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from("sleep 5")],
        };
        let executor = RuntimeExecutor::new(RuntimeExecutionPolicy {
            timeout: Duration::from_millis(50),
            poll_interval: Duration::from_millis(5),
            ..RuntimeExecutionPolicy::default()
        });

        assert!(matches!(
            executor.execute(&command),
            Err(RuntimeExecutionError::TimedOut { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_nonzero_exit_and_preserves_bounded_stderr() {
        let command = RuntimeCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                OsString::from("-c"),
                OsString::from("printf 'failure' >&2; exit 7"),
            ],
        };

        let error = executor_error(&command);
        match error {
            RuntimeExecutionError::NonZeroExit { code, stderr, .. } => {
                assert_eq!(code, Some(7));
                assert_eq!(stderr.text_lossy(), "failure");
            }
            other => panic!("expected non-zero exit, got {other}"),
        }
    }

    #[cfg(unix)]
    fn executor_error(command: &RuntimeCommand) -> RuntimeExecutionError {
        match RuntimeExecutor::default().execute(command) {
            Ok(_) => panic!("command should fail"),
            Err(error) => error,
        }
    }
}
