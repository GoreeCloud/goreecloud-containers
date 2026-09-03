use goreecloud_containers_core::ContainerId;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

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
        write!(formatter, "unsupported OCI runtime '{}'; expected 'crun' or 'runc'", self.0)
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
    Spawn { executable: PathBuf, source: io::Error },
    NonZeroExit { executable: PathBuf, code: Option<i32>, stderr: String },
    EmptyVersionOutput { executable: PathBuf },
    BundlePathNotAbsolute(PathBuf),
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
            Self::BundlePathNotAbsolute(path) => write!(
                formatter,
                "OCI bundle path must be absolute to avoid working-directory ambiguity: {}",
                path.display()
            ),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            _ => None,
        }
    }
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
            return Err(RuntimeError::BundlePathNotAbsolute(bundle_path.to_path_buf()));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> ContainerId {
        match ContainerId::parse("example") {
            Ok(id) => id,
            Err(error) => panic!("test identifier should be valid: {error}"),
        }
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
        assert_eq!(runtime.plan_start(&id()).display_lossy(), "runc start example");
        assert_eq!(runtime.plan_state(&id()).display_lossy(), "runc state example");
        assert_eq!(runtime.plan_delete(&id()).display_lossy(), "runc delete example");
    }
}
