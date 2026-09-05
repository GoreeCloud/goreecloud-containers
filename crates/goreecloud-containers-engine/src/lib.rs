use goreecloud_containers_image::ContentStore;
use goreecloud_containers_image::registry::{
    ImageConfiguration, PulledImage, RegistryClient, RegistryError, RegistryReference,
};
use goreecloud_containers_image::rootfs::RootfsPolicy;
use goreecloud_containers_oci::{
    DEFAULT_ROOTFS_DIRECTORY, InitializedBundle, OciConfig, OciConfigError, initialize_linux_bundle,
};
use std::collections::HashSet;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const OCI_CONFIG_FILENAME: &str = "config.json";
static BUNDLE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct PulledBundle {
    pub image: PulledImage,
    pub bundle: InitializedBundle,
}

#[derive(Debug)]
pub enum EngineError {
    UnsupportedImageOperatingSystem(String),
    MissingImageCommand,
    InvalidImageEnvironment(String),
    InvalidImageWorkingDirectory(String),
    UnsupportedImageUser(String),
    PrivilegedImageUser(String),
    BundleTargetNotAbsolute(PathBuf),
    BundleTargetHasNoParent(PathBuf),
    BundleTargetHasInvalidName(PathBuf),
    BundleParentSymlink(PathBuf),
    BundleParentNotDirectory(PathBuf),
    BundleParentNotCanonical {
        requested: PathBuf,
        resolved: PathBuf,
    },
    BundleTargetAlreadyExists(PathBuf),
    PulledRootfsMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    PulledLayerCountMismatch {
        layers: usize,
        applied_layers: usize,
        diff_ids: usize,
    },
    Registry(Box<RegistryError>),
    Oci(Box<OciConfigError>),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedImageOperatingSystem(value) => {
                write!(formatter, "unsupported image operating system: {value}")
            }
            Self::MissingImageCommand => formatter.write_str(
                "image configuration does not provide a supported Entrypoint or Cmd command",
            ),
            Self::InvalidImageEnvironment(value) => {
                write!(formatter, "invalid image environment entry: {value}")
            }
            Self::InvalidImageWorkingDirectory(value) => write!(
                formatter,
                "image working directory must be an absolute normalized Linux path: {value}"
            ),
            Self::UnsupportedImageUser(value) => write!(
                formatter,
                "unsupported image user '{value}'; Development bundle integration accepts only a non-zero numeric UID:GID pair"
            ),
            Self::PrivilegedImageUser(value) => write!(
                formatter,
                "image user '{value}' requests UID or GID 0; privileged image users are not accepted by the current Development bundle policy"
            ),
            Self::BundleTargetNotAbsolute(path) => write!(
                formatter,
                "OCI bundle target path must be absolute: {}",
                path.display()
            ),
            Self::BundleTargetHasNoParent(path) => write!(
                formatter,
                "OCI bundle target path has no parent: {}",
                path.display()
            ),
            Self::BundleTargetHasInvalidName(path) => write!(
                formatter,
                "OCI bundle target path has an invalid final component: {}",
                path.display()
            ),
            Self::BundleParentSymlink(path) => write!(
                formatter,
                "OCI bundle target parent must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::BundleParentNotDirectory(path) => write!(
                formatter,
                "OCI bundle target parent is not a directory: {}",
                path.display()
            ),
            Self::BundleParentNotCanonical {
                requested,
                resolved,
            } => write!(
                formatter,
                "OCI bundle target parent must be canonical; requested '{}', resolved '{}'",
                requested.display(),
                resolved.display()
            ),
            Self::BundleTargetAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing OCI bundle target: {}",
                path.display()
            ),
            Self::PulledRootfsMismatch { expected, actual } => write!(
                formatter,
                "pulled rootfs path does not match the controlled bundle staging path; expected '{}', got '{}'",
                expected.display(),
                actual.display()
            ),
            Self::PulledLayerCountMismatch {
                layers,
                applied_layers,
                diff_ids,
            } => write!(
                formatter,
                "pulled image layer accounting is inconsistent: {layers} layer records, {applied_layers} applied layers, {diff_ids} diff IDs"
            ),
            Self::Registry(error) => error.fmt(formatter),
            Self::Oci(error) => error.fmt(formatter),
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

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error.as_ref()),
            Self::Oci(error) => Some(error.as_ref()),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<RegistryError> for EngineError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(Box::new(error))
    }
}

impl From<OciConfigError> for EngineError {
    fn from(error: OciConfigError) -> Self {
        Self::Oci(Box::new(error))
    }
}

pub fn oci_config_from_image_configuration(
    image: &ImageConfiguration,
) -> Result<OciConfig, EngineError> {
    if image.os != "linux" {
        return Err(EngineError::UnsupportedImageOperatingSystem(
            image.os.clone(),
        ));
    }

    let mut process_args = image.process.entrypoint.clone();
    process_args.extend(image.process.cmd.iter().cloned());
    if process_args.is_empty() {
        return Err(EngineError::MissingImageCommand);
    }

    let mut config = OciConfig::minimal_linux(process_args)?;

    if !image.process.env.is_empty() {
        validate_image_environment(&image.process.env)?;
        config.process = config.process.clone().with_env(image.process.env.clone())?;
    }

    if let Some(working_dir) = image
        .process
        .working_dir
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        validate_image_working_directory(working_dir)?;
        config.process = config.process.clone().with_cwd(working_dir.to_owned())?;
    }

    if let Some(user) = image
        .process
        .user
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        let (uid, gid) = parse_image_user(user)?;
        config.process = config.process.clone().with_user(uid, gid);
    }

    Ok(config)
}

pub fn pull_image_to_bundle(
    reference: &RegistryReference,
    content_store: &ContentStore,
    bundle_target: &Path,
    rootfs_policy: RootfsPolicy,
) -> Result<PulledBundle, EngineError> {
    let (parent, target_name) = validate_new_bundle_target(bundle_target)?;
    let stage = create_staging_directory(&parent)?;
    let stage_rootfs = stage.join(DEFAULT_ROOTFS_DIRECTORY);

    let result = (|| {
        let pulled = RegistryClient::new().pull_image(
            reference,
            content_store,
            &stage_rootfs,
            rootfs_policy,
        )?;
        finalize_pulled_image_bundle(&stage, bundle_target, &target_name, pulled)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn finalize_pulled_image_bundle(
    stage: &Path,
    bundle_target: &Path,
    target_name: &OsStr,
    mut pulled: PulledImage,
) -> Result<PulledBundle, EngineError> {
    let expected_rootfs = stage.join(DEFAULT_ROOTFS_DIRECTORY);
    if pulled.rootfs.rootfs_path != expected_rootfs {
        return Err(EngineError::PulledRootfsMismatch {
            expected: expected_rootfs,
            actual: pulled.rootfs.rootfs_path,
        });
    }
    if pulled.layers.len() != pulled.rootfs.applied_layers
        || pulled.layers.len() != pulled.configuration.diff_ids.len()
    {
        return Err(EngineError::PulledLayerCountMismatch {
            layers: pulled.layers.len(),
            applied_layers: pulled.rootfs.applied_layers,
            diff_ids: pulled.configuration.diff_ids.len(),
        });
    }

    let config = oci_config_from_image_configuration(&pulled.configuration)?;
    let initialized_stage = initialize_linux_bundle(stage, &config)?;
    let bundle = publish_staged_bundle(&initialized_stage, bundle_target, target_name)?;
    pulled.rootfs.rootfs_path = bundle.rootfs_path.clone();

    Ok(PulledBundle {
        image: pulled,
        bundle,
    })
}

fn validate_image_environment(entries: &[String]) -> Result<(), EngineError> {
    let mut names = HashSet::with_capacity(entries.len());
    for entry in entries {
        if entry.contains('\0') {
            return Err(EngineError::InvalidImageEnvironment(entry.clone()));
        }
        let Some((name, _value)) = entry.split_once('=') else {
            return Err(EngineError::InvalidImageEnvironment(entry.clone()));
        };
        if !is_conservative_environment_name(name) || !names.insert(name) {
            return Err(EngineError::InvalidImageEnvironment(entry.clone()));
        }
    }
    Ok(())
}

fn is_conservative_environment_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|value| value.is_ascii_alphanumeric() || value == b'_')
}

fn validate_image_working_directory(value: &str) -> Result<(), EngineError> {
    if value.contains('\0') || !value.starts_with('/') {
        return Err(EngineError::InvalidImageWorkingDirectory(value.to_owned()));
    }
    if value
        .split('/')
        .any(|component| matches!(component, "." | ".."))
    {
        return Err(EngineError::InvalidImageWorkingDirectory(value.to_owned()));
    }
    Ok(())
}

fn parse_image_user(value: &str) -> Result<(u32, u32), EngineError> {
    if value.contains('\0') || value.trim() != value {
        return Err(EngineError::UnsupportedImageUser(value.to_owned()));
    }
    let Some((uid, gid)) = value.split_once(':') else {
        return Err(EngineError::UnsupportedImageUser(value.to_owned()));
    };
    if uid.is_empty()
        || gid.is_empty()
        || gid.contains(':')
        || !uid.bytes().all(|byte| byte.is_ascii_digit())
        || !gid.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(EngineError::UnsupportedImageUser(value.to_owned()));
    }
    let uid = uid
        .parse::<u32>()
        .map_err(|_| EngineError::UnsupportedImageUser(value.to_owned()))?;
    let gid = gid
        .parse::<u32>()
        .map_err(|_| EngineError::UnsupportedImageUser(value.to_owned()))?;
    if uid == 0 || gid == 0 {
        return Err(EngineError::PrivilegedImageUser(value.to_owned()));
    }
    Ok((uid, gid))
}

fn validate_new_bundle_target(bundle_target: &Path) -> Result<(PathBuf, OsString), EngineError> {
    if !bundle_target.is_absolute() {
        return Err(EngineError::BundleTargetNotAbsolute(
            bundle_target.to_path_buf(),
        ));
    }
    let parent = bundle_target
        .parent()
        .ok_or_else(|| EngineError::BundleTargetHasNoParent(bundle_target.to_path_buf()))?;
    let target_name = bundle_target
        .file_name()
        .filter(|name| *name != OsStr::new(".") && *name != OsStr::new(".."))
        .ok_or_else(|| EngineError::BundleTargetHasInvalidName(bundle_target.to_path_buf()))?
        .to_os_string();

    let parent_metadata = fs::symlink_metadata(parent).map_err(|source| EngineError::Io {
        operation: "inspect OCI bundle target parent",
        path: parent.to_path_buf(),
        source,
    })?;
    if parent_metadata.file_type().is_symlink() {
        return Err(EngineError::BundleParentSymlink(parent.to_path_buf()));
    }
    if !parent_metadata.is_dir() {
        return Err(EngineError::BundleParentNotDirectory(parent.to_path_buf()));
    }
    let canonical_parent = fs::canonicalize(parent).map_err(|source| EngineError::Io {
        operation: "canonicalize OCI bundle target parent",
        path: parent.to_path_buf(),
        source,
    })?;
    if canonical_parent != parent {
        return Err(EngineError::BundleParentNotCanonical {
            requested: parent.to_path_buf(),
            resolved: canonical_parent,
        });
    }

    match fs::symlink_metadata(bundle_target) {
        Ok(_) => {
            return Err(EngineError::BundleTargetAlreadyExists(
                bundle_target.to_path_buf(),
            ));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(EngineError::Io {
                operation: "inspect OCI bundle target",
                path: bundle_target.to_path_buf(),
                source,
            });
        }
    }

    Ok((canonical_parent, target_name))
}

fn create_staging_directory(parent: &Path) -> Result<PathBuf, EngineError> {
    for _ in 0..32 {
        let sequence = BUNDLE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stage = parent.join(format!(
            ".goreecloud-bundle-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&stage) {
            Ok(()) => {
                set_private_directory_permissions(&stage)?;
                return Ok(stage);
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(EngineError::Io {
                    operation: "create OCI bundle staging directory",
                    path: stage,
                    source,
                });
            }
        }
    }
    Err(EngineError::Io {
        operation: "allocate unique OCI bundle staging directory",
        path: parent.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique OCI bundle staging directory",
        ),
    })
}

fn publish_staged_bundle(
    staged: &InitializedBundle,
    bundle_target: &Path,
    target_name: &OsStr,
) -> Result<InitializedBundle, EngineError> {
    let parent = bundle_target
        .parent()
        .ok_or_else(|| EngineError::BundleTargetHasNoParent(bundle_target.to_path_buf()))?;
    if bundle_target.file_name() != Some(target_name) {
        return Err(EngineError::BundleTargetHasInvalidName(
            bundle_target.to_path_buf(),
        ));
    }

    fs::create_dir(bundle_target).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            EngineError::BundleTargetAlreadyExists(bundle_target.to_path_buf())
        } else {
            EngineError::Io {
                operation: "create OCI bundle target",
                path: bundle_target.to_path_buf(),
                source,
            }
        }
    })?;
    if let Err(error) = set_private_directory_permissions(bundle_target) {
        let _ = fs::remove_dir(bundle_target);
        return Err(error);
    }

    let target_rootfs = bundle_target.join(DEFAULT_ROOTFS_DIRECTORY);
    let target_config = bundle_target.join(OCI_CONFIG_FILENAME);
    let mut rootfs_moved = false;
    let mut config_moved = false;

    let publish_result = (|| {
        fs::rename(&staged.rootfs_path, &target_rootfs).map_err(|source| EngineError::Io {
            operation: "publish OCI root filesystem",
            path: target_rootfs.clone(),
            source,
        })?;
        rootfs_moved = true;
        fs::rename(&staged.config_path, &target_config).map_err(|source| EngineError::Io {
            operation: "publish OCI configuration",
            path: target_config.clone(),
            source,
        })?;
        config_moved = true;
        fs::remove_dir(&staged.bundle_path).map_err(|source| EngineError::Io {
            operation: "remove empty OCI bundle staging directory",
            path: staged.bundle_path.clone(),
            source,
        })?;
        Ok(())
    })();

    if let Err(error) = publish_result {
        if config_moved {
            let _ = fs::remove_file(&target_config);
        }
        if rootfs_moved {
            let _ = fs::remove_dir_all(&target_rootfs);
        }
        let _ = fs::remove_dir(bundle_target);
        return Err(error);
    }

    Ok(InitializedBundle {
        bundle_path: parent.join(target_name),
        rootfs_path: target_rootfs,
        config_path: target_config,
    })
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), EngineError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| EngineError::Io {
        operation: "set private OCI bundle directory permissions",
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), EngineError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use goreecloud_containers_image::Sha256Digest;
    use goreecloud_containers_image::registry::ImageProcessConfig;
    use goreecloud_containers_image::rootfs::RootfsBuild;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn image_configuration(process: ImageProcessConfig) -> ImageConfiguration {
        ImageConfiguration {
            architecture: "amd64".to_owned(),
            os: "linux".to_owned(),
            diff_ids: Vec::new(),
            process,
        }
    }

    fn default_process() -> ImageProcessConfig {
        ImageProcessConfig {
            user: None,
            env: Vec::new(),
            entrypoint: Vec::new(),
            cmd: vec!["/bin/echo".to_owned(), "hello".to_owned()],
            working_dir: None,
        }
    }

    fn test_parent(name: &str) -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "goreecloud-containers-engine-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test parent should be created");
        fs::canonicalize(path).expect("test parent should canonicalize")
    }

    fn zero_digest() -> Sha256Digest {
        format!("sha256:{}", "0".repeat(64))
            .parse()
            .expect("test digest should parse")
    }

    #[test]
    fn maps_entrypoint_and_cmd_without_shell_interpretation() {
        let mut process = default_process();
        process.entrypoint = vec!["/usr/bin/example".to_owned(), "--fixed".to_owned()];
        process.cmd = vec!["value with spaces".to_owned()];
        let config = oci_config_from_image_configuration(&image_configuration(process))
            .expect("supported image process should map");
        assert_eq!(
            config.process.args,
            vec!["/usr/bin/example", "--fixed", "value with spaces"]
        );
        assert!(config.process.no_new_privileges);
    }

    #[test]
    fn keeps_safe_defaults_when_optional_image_process_fields_are_absent() {
        let config = oci_config_from_image_configuration(&image_configuration(default_process()))
            .expect("default process should map");
        assert_eq!(config.process.user.uid, 65_534);
        assert_eq!(config.process.user.gid, 65_534);
        assert_eq!(config.process.cwd, "/");
        assert_eq!(
            config.process.env,
            vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"]
        );
    }

    #[test]
    fn rejects_missing_image_command() {
        let process = ImageProcessConfig {
            cmd: Vec::new(),
            ..default_process()
        };
        assert!(matches!(
            oci_config_from_image_configuration(&image_configuration(process)),
            Err(EngineError::MissingImageCommand)
        ));
    }

    #[test]
    fn validates_environment_working_directory_and_numeric_user() {
        let mut process = default_process();
        process.env = vec!["HOME=/work".to_owned(), "MODE=development".to_owned()];
        process.working_dir = Some("/work/app".to_owned());
        process.user = Some("1000:1001".to_owned());
        let config = oci_config_from_image_configuration(&image_configuration(process))
            .expect("supported image process metadata should map");
        assert_eq!(config.process.env, vec!["HOME=/work", "MODE=development"]);
        assert_eq!(config.process.cwd, "/work/app");
        assert_eq!(config.process.user.uid, 1000);
        assert_eq!(config.process.user.gid, 1001);
        assert!(config.process.no_new_privileges);
    }

    #[test]
    fn rejects_ambiguous_or_privileged_image_process_metadata() {
        let mut duplicate_env = default_process();
        duplicate_env.env = vec!["MODE=a".to_owned(), "MODE=b".to_owned()];
        assert!(matches!(
            oci_config_from_image_configuration(&image_configuration(duplicate_env)),
            Err(EngineError::InvalidImageEnvironment(_))
        ));

        let mut relative_cwd = default_process();
        relative_cwd.working_dir = Some("work".to_owned());
        assert!(matches!(
            oci_config_from_image_configuration(&image_configuration(relative_cwd)),
            Err(EngineError::InvalidImageWorkingDirectory(_))
        ));

        let mut named_user = default_process();
        named_user.user = Some("app".to_owned());
        assert!(matches!(
            oci_config_from_image_configuration(&image_configuration(named_user)),
            Err(EngineError::UnsupportedImageUser(_))
        ));

        let mut root_user = default_process();
        root_user.user = Some("0:0".to_owned());
        assert!(matches!(
            oci_config_from_image_configuration(&image_configuration(root_user)),
            Err(EngineError::PrivilegedImageUser(_))
        ));
    }

    #[test]
    fn publishes_a_prepared_verified_rootfs_without_overwriting_target() {
        let parent = test_parent("publish");
        let target = parent.join("bundle");
        let stage = create_staging_directory(&parent).expect("stage should be created");
        let stage_rootfs = stage.join(DEFAULT_ROOTFS_DIRECTORY);
        fs::create_dir(&stage_rootfs).expect("stage rootfs should be created");
        fs::write(stage_rootfs.join("marker"), b"verified fixture")
            .expect("fixture marker should be written");

        let pulled = PulledImage {
            manifest_digest: zero_digest(),
            config_digest: zero_digest(),
            layers: Vec::new(),
            rootfs: RootfsBuild {
                rootfs_path: stage_rootfs,
                applied_layers: 0,
            },
            configuration: image_configuration(default_process()),
        };
        let bundle = finalize_pulled_image_bundle(&stage, &target, OsStr::new("bundle"), pulled)
            .expect("prepared verified rootfs should publish as a bundle");
        assert_eq!(bundle.bundle.bundle_path, target);
        assert!(bundle.bundle.rootfs_path.join("marker").is_file());
        assert!(bundle.bundle.config_path.is_file());
        assert!(!stage.exists());
        assert_eq!(bundle.image.rootfs.rootfs_path, bundle.bundle.rootfs_path);

        let existing_result = validate_new_bundle_target(&bundle.bundle.bundle_path);
        assert!(matches!(
            existing_result,
            Err(EngineError::BundleTargetAlreadyExists(_))
        ));
        fs::remove_dir_all(&parent).expect("test parent should be removed");
    }

    #[test]
    fn refuses_relative_bundle_targets() {
        assert!(matches!(
            validate_new_bundle_target(Path::new("relative-bundle")),
            Err(EngineError::BundleTargetNotAbsolute(_))
        ));
    }
}
