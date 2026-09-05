use crate::Sha256Digest;
use flate2::read::GzDecoder;
use sha2::{Digest as _, Sha256};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const OCI_LAYER_TAR: &str = "application/vnd.oci.image.layer.v1.tar";
pub const OCI_LAYER_TAR_GZIP: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
pub const DOCKER_LAYER_TAR_GZIP: &str = "application/vnd.docker.image.rootfs.diff.tar.gzip";

static ROOTFS_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootfsPolicy {
    pub max_unpacked_bytes_per_layer: u64,
    pub max_entry_bytes: u64,
    pub max_entries_per_layer: usize,
}

impl Default for RootfsPolicy {
    fn default() -> Self {
        Self {
            max_unpacked_bytes_per_layer: 4 * 1024 * 1024 * 1024,
            max_entry_bytes: 1024 * 1024 * 1024,
            max_entries_per_layer: 200_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LayerArchive {
    pub media_type: String,
    pub path: PathBuf,
    pub expected_diff_id: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootfsBuild {
    pub rootfs_path: PathBuf,
    pub applied_layers: usize,
}

#[derive(Debug)]
pub enum RootfsError {
    RootfsPathNotAbsolute(PathBuf),
    RootfsPathHasNoParent(PathBuf),
    RootfsPathHasInvalidName(PathBuf),
    RootfsParentNotCanonical {
        requested: PathBuf,
        resolved: PathBuf,
    },
    RootfsTargetAlreadyExists(PathBuf),
    RootfsParentNotDirectory(PathBuf),
    RootfsParentSymlink(PathBuf),
    LayerPathNotAbsolute(PathBuf),
    LayerPathSymlink(PathBuf),
    LayerNotRegularFile(PathBuf),
    UnsupportedLayerMediaType(String),
    LayerTooLarge {
        maximum: u64,
    },
    LayerDiffIdMismatch {
        expected: Sha256Digest,
        actual: Sha256Digest,
    },
    TooManyEntries {
        maximum: usize,
    },
    EntryTooLarge {
        path: PathBuf,
        maximum: u64,
    },
    UnsafeArchivePath(PathBuf),
    UnsupportedEntryType {
        path: PathBuf,
        entry_type: u8,
    },
    ParentPathSymlink(PathBuf),
    ParentPathNotDirectory(PathBuf),
    TargetPathSymlink(PathBuf),
    TargetPathIsDirectory(PathBuf),
    TargetPathNotDirectory(PathBuf),
    InvalidWhiteout(PathBuf),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RootfsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootfsPathNotAbsolute(path) => {
                write!(
                    formatter,
                    "rootfs target path must be absolute: {}",
                    path.display()
                )
            }
            Self::RootfsPathHasNoParent(path) => {
                write!(
                    formatter,
                    "rootfs target path has no parent: {}",
                    path.display()
                )
            }
            Self::RootfsPathHasInvalidName(path) => write!(
                formatter,
                "rootfs target must end in one normal path component: {}",
                path.display()
            ),
            Self::RootfsParentNotCanonical {
                requested,
                resolved,
            } => write!(
                formatter,
                "rootfs target path must not traverse symbolic-link or non-canonical parents: requested '{}', resolved '{}'",
                requested.display(),
                resolved.display()
            ),
            Self::RootfsTargetAlreadyExists(path) => write!(
                formatter,
                "rootfs target must not already exist; refusing to merge into '{}': {}",
                path.display(),
                path.display()
            ),
            Self::RootfsParentNotDirectory(path) => {
                write!(
                    formatter,
                    "rootfs parent is not a directory: {}",
                    path.display()
                )
            }
            Self::RootfsParentSymlink(path) => write!(
                formatter,
                "rootfs parent must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::LayerPathNotAbsolute(path) => {
                write!(formatter, "layer path must be absolute: {}", path.display())
            }
            Self::LayerPathSymlink(path) => write!(
                formatter,
                "layer path must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::LayerNotRegularFile(path) => {
                write!(formatter, "layer is not a regular file: {}", path.display())
            }
            Self::UnsupportedLayerMediaType(media_type) => {
                write!(formatter, "unsupported OCI layer media type: {media_type}")
            }
            Self::LayerTooLarge { maximum } => write!(
                formatter,
                "uncompressed layer exceeds configured limit of {maximum} bytes"
            ),
            Self::LayerDiffIdMismatch { expected, actual } => write!(
                formatter,
                "uncompressed layer digest mismatch: expected {expected}, calculated {actual}"
            ),
            Self::TooManyEntries { maximum } => write!(
                formatter,
                "layer contains more than the configured maximum of {maximum} archive entries"
            ),
            Self::EntryTooLarge { path, maximum } => write!(
                formatter,
                "archive entry '{}' exceeds configured per-entry limit of {maximum} bytes",
                path.display()
            ),
            Self::UnsafeArchivePath(path) => {
                write!(
                    formatter,
                    "unsafe archive path rejected: {}",
                    path.display()
                )
            }
            Self::UnsupportedEntryType { path, entry_type } => write!(
                formatter,
                "unsupported archive entry type 0x{entry_type:02x} at '{}'",
                path.display()
            ),
            Self::ParentPathSymlink(path) => write!(
                formatter,
                "archive extraction would traverse symbolic-link parent: {}",
                path.display()
            ),
            Self::ParentPathNotDirectory(path) => write!(
                formatter,
                "archive extraction parent is not a directory: {}",
                path.display()
            ),
            Self::TargetPathSymlink(path) => write!(
                formatter,
                "archive extraction refuses to replace symbolic-link target: {}",
                path.display()
            ),
            Self::TargetPathIsDirectory(path) => write!(
                formatter,
                "archive file entry conflicts with existing directory: {}",
                path.display()
            ),
            Self::TargetPathNotDirectory(path) => write!(
                formatter,
                "archive directory entry conflicts with existing non-directory: {}",
                path.display()
            ),
            Self::InvalidWhiteout(path) => {
                write!(formatter, "invalid OCI whiteout entry: {}", path.display())
            }
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

impl Error for RootfsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub fn build_rootfs(
    target: &Path,
    layers: &[LayerArchive],
    policy: RootfsPolicy,
) -> Result<RootfsBuild, RootfsError> {
    if !target.is_absolute() {
        return Err(RootfsError::RootfsPathNotAbsolute(target.to_path_buf()));
    }

    let parent = target
        .parent()
        .ok_or_else(|| RootfsError::RootfsPathHasNoParent(target.to_path_buf()))?;
    let name = target
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| RootfsError::RootfsPathHasInvalidName(target.to_path_buf()))?;

    let parent_metadata = fs::symlink_metadata(parent).map_err(|source| RootfsError::Io {
        operation: "inspect rootfs parent",
        path: parent.to_path_buf(),
        source,
    })?;
    if parent_metadata.file_type().is_symlink() {
        return Err(RootfsError::RootfsParentSymlink(parent.to_path_buf()));
    }
    if !parent_metadata.is_dir() {
        return Err(RootfsError::RootfsParentNotDirectory(parent.to_path_buf()));
    }

    let canonical_parent = fs::canonicalize(parent).map_err(|source| RootfsError::Io {
        operation: "canonicalize rootfs parent",
        path: parent.to_path_buf(),
        source,
    })?;
    let resolved_target = canonical_parent.join(name);
    if resolved_target != target {
        return Err(RootfsError::RootfsParentNotCanonical {
            requested: target.to_path_buf(),
            resolved: resolved_target,
        });
    }

    match fs::symlink_metadata(target) {
        Ok(_) => return Err(RootfsError::RootfsTargetAlreadyExists(target.to_path_buf())),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(RootfsError::Io {
                operation: "inspect rootfs target",
                path: target.to_path_buf(),
                source,
            });
        }
    }

    for layer in layers {
        validate_layer_source(layer)?;
        verify_layer_diff_id(layer, policy.max_unpacked_bytes_per_layer)?;
    }

    let staging = create_staging_directory(&canonical_parent)?;
    let extraction_result = (|| {
        for layer in layers {
            apply_layer(&staging, layer, policy)?;
        }
        Ok::<(), RootfsError>(())
    })();

    if let Err(error) = extraction_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    if let Err(source) = fs::rename(&staging, target) {
        let _ = fs::remove_dir_all(&staging);
        return Err(RootfsError::Io {
            operation: "publish completed rootfs",
            path: target.to_path_buf(),
            source,
        });
    }

    Ok(RootfsBuild {
        rootfs_path: target.to_path_buf(),
        applied_layers: layers.len(),
    })
}

fn validate_layer_source(layer: &LayerArchive) -> Result<(), RootfsError> {
    if !layer.path.is_absolute() {
        return Err(RootfsError::LayerPathNotAbsolute(layer.path.clone()));
    }
    let metadata = fs::symlink_metadata(&layer.path).map_err(|source| RootfsError::Io {
        operation: "inspect layer archive",
        path: layer.path.clone(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RootfsError::LayerPathSymlink(layer.path.clone()));
    }
    if !metadata.is_file() {
        return Err(RootfsError::LayerNotRegularFile(layer.path.clone()));
    }
    if !is_supported_layer_media_type(&layer.media_type) {
        return Err(RootfsError::UnsupportedLayerMediaType(
            layer.media_type.clone(),
        ));
    }
    Ok(())
}

#[must_use]
pub fn is_supported_layer_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        OCI_LAYER_TAR | OCI_LAYER_TAR_GZIP | DOCKER_LAYER_TAR_GZIP
    )
}

fn verify_layer_diff_id(layer: &LayerArchive, maximum: u64) -> Result<(), RootfsError> {
    let file = File::open(&layer.path).map_err(|source| RootfsError::Io {
        operation: "open layer for uncompressed digest verification",
        path: layer.path.clone(),
        source,
    })?;

    let actual = match layer.media_type.as_str() {
        OCI_LAYER_TAR => hash_reader_bounded(file, maximum)?,
        OCI_LAYER_TAR_GZIP | DOCKER_LAYER_TAR_GZIP => {
            hash_reader_bounded(GzDecoder::new(file), maximum)?
        }
        other => return Err(RootfsError::UnsupportedLayerMediaType(other.to_owned())),
    };

    if actual != layer.expected_diff_id {
        return Err(RootfsError::LayerDiffIdMismatch {
            expected: layer.expected_diff_id,
            actual,
        });
    }
    Ok(())
}

fn hash_reader_bounded(mut reader: impl Read, maximum: u64) -> Result<Sha256Digest, RootfsError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|source| RootfsError::Io {
            operation: "read uncompressed layer",
            path: PathBuf::from("<layer-reader>"),
            source,
        })?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).expect("read buffer length fits in u64"))
            .ok_or(RootfsError::LayerTooLarge { maximum })?;
        if total > maximum {
            return Err(RootfsError::LayerTooLarge { maximum });
        }
        hasher.update(&buffer[..count]);
    }
    Ok(Sha256Digest::from_bytes(hasher.finalize().into()))
}

fn apply_layer(
    rootfs: &Path,
    layer: &LayerArchive,
    policy: RootfsPolicy,
) -> Result<(), RootfsError> {
    let file = File::open(&layer.path).map_err(|source| RootfsError::Io {
        operation: "open verified layer for extraction",
        path: layer.path.clone(),
        source,
    })?;

    match layer.media_type.as_str() {
        OCI_LAYER_TAR => apply_tar(rootfs, file, policy),
        OCI_LAYER_TAR_GZIP | DOCKER_LAYER_TAR_GZIP => {
            apply_tar(rootfs, GzDecoder::new(file), policy)
        }
        other => Err(RootfsError::UnsupportedLayerMediaType(other.to_owned())),
    }
}

fn apply_tar(rootfs: &Path, reader: impl Read, policy: RootfsPolicy) -> Result<(), RootfsError> {
    let mut archive = tar::Archive::new(reader);
    let entries = archive.entries().map_err(|source| RootfsError::Io {
        operation: "read layer archive entries",
        path: PathBuf::from("<tar>"),
        source,
    })?;

    let mut entry_count = 0_usize;
    let mut declared_bytes = 0_u64;
    for entry_result in entries {
        entry_count = entry_count
            .checked_add(1)
            .ok_or(RootfsError::TooManyEntries {
                maximum: policy.max_entries_per_layer,
            })?;
        if entry_count > policy.max_entries_per_layer {
            return Err(RootfsError::TooManyEntries {
                maximum: policy.max_entries_per_layer,
            });
        }

        let mut entry = entry_result.map_err(|source| RootfsError::Io {
            operation: "read layer archive entry",
            path: PathBuf::from("<tar-entry>"),
            source,
        })?;
        let archive_path = entry.path().map_err(|source| RootfsError::Io {
            operation: "decode layer archive path",
            path: PathBuf::from("<tar-entry>"),
            source,
        })?;
        let relative = normalize_archive_path(&archive_path)?;
        let size = entry.header().size().map_err(|source| RootfsError::Io {
            operation: "read layer archive entry size",
            path: relative.clone(),
            source,
        })?;
        if size > policy.max_entry_bytes {
            return Err(RootfsError::EntryTooLarge {
                path: relative,
                maximum: policy.max_entry_bytes,
            });
        }
        declared_bytes = declared_bytes
            .checked_add(size)
            .ok_or(RootfsError::LayerTooLarge {
                maximum: policy.max_unpacked_bytes_per_layer,
            })?;
        if declared_bytes > policy.max_unpacked_bytes_per_layer {
            return Err(RootfsError::LayerTooLarge {
                maximum: policy.max_unpacked_bytes_per_layer,
            });
        }

        if is_whiteout_path(&relative) {
            apply_whiteout(rootfs, &relative)?;
            continue;
        }

        let entry_type = entry.header().entry_type();
        match entry_type {
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                let mode = entry.header().mode().map_err(|source| RootfsError::Io {
                    operation: "read layer archive file mode",
                    path: relative.clone(),
                    source,
                })?;
                write_regular_file(rootfs, &relative, &mut entry, size, mode)?;
            }
            tar::EntryType::Directory => create_directory(rootfs, &relative)?,
            _ => {
                return Err(RootfsError::UnsupportedEntryType {
                    path: relative,
                    entry_type: entry_type.as_byte(),
                });
            }
        }
    }
    Ok(())
}

fn normalize_archive_path(path: &Path) -> Result<PathBuf, RootfsError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RootfsError::UnsafeArchivePath(path.to_path_buf()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(RootfsError::UnsafeArchivePath(path.to_path_buf()));
    }
    Ok(normalized)
}

fn is_whiteout_path(relative: &Path) -> bool {
    relative
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with(".wh."))
}

fn apply_whiteout(rootfs: &Path, relative: &Path) -> Result<(), RootfsError> {
    let name = relative
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| RootfsError::InvalidWhiteout(relative.to_path_buf()))?;
    let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
    let parent = ensure_directory_chain(rootfs, parent_relative)?;

    if name == ".wh..wh..opq" {
        for child in fs::read_dir(&parent).map_err(|source| RootfsError::Io {
            operation: "read opaque whiteout directory",
            path: parent.clone(),
            source,
        })? {
            let child = child.map_err(|source| RootfsError::Io {
                operation: "read opaque whiteout child",
                path: parent.clone(),
                source,
            })?;
            remove_without_following(&child.path())?;
        }
        return Ok(());
    }

    let target_name = name
        .strip_prefix(".wh.")
        .filter(|value| !value.is_empty() && *value != "." && *value != "..")
        .ok_or_else(|| RootfsError::InvalidWhiteout(relative.to_path_buf()))?;
    remove_without_following(&parent.join(target_name))
}

fn create_directory(rootfs: &Path, relative: &Path) -> Result<(), RootfsError> {
    let _ = ensure_directory_chain(rootfs, relative)?;
    Ok(())
}

fn ensure_directory_chain(rootfs: &Path, relative: &Path) -> Result<PathBuf, RootfsError> {
    let mut current = rootfs.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(RootfsError::UnsafeArchivePath(relative.to_path_buf()));
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(RootfsError::ParentPathSymlink(current));
                }
                if !metadata.is_dir() {
                    return Err(RootfsError::ParentPathNotDirectory(current));
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|source| RootfsError::Io {
                    operation: "create rootfs directory",
                    path: current.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(RootfsError::Io {
                    operation: "inspect rootfs directory",
                    path: current,
                    source,
                });
            }
        }
    }
    Ok(current)
}

fn write_regular_file(
    rootfs: &Path,
    relative: &Path,
    entry: &mut impl Read,
    expected_size: u64,
    mode: u32,
) -> Result<(), RootfsError> {
    let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
    let parent = ensure_directory_chain(rootfs, parent_relative)?;
    let file_name = relative
        .file_name()
        .ok_or_else(|| RootfsError::UnsafeArchivePath(relative.to_path_buf()))?;
    let target = parent.join(file_name);

    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(RootfsError::TargetPathSymlink(target));
        }
        Ok(metadata) if metadata.is_dir() => {
            return Err(RootfsError::TargetPathIsDirectory(target));
        }
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(RootfsError::Io {
                operation: "inspect rootfs file target",
                path: target,
                source,
            });
        }
    }

    let (temporary, mut output) = create_temporary_file(&parent)?;
    let copied = io::copy(entry, &mut output).map_err(|source| RootfsError::Io {
        operation: "extract regular file",
        path: temporary.clone(),
        source,
    })?;
    if copied != expected_size {
        let _ = fs::remove_file(&temporary);
        return Err(RootfsError::Io {
            operation: "extract complete regular file",
            path: target,
            source: io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("expected {expected_size} bytes, extracted {copied}"),
            ),
        });
    }
    output.flush().map_err(|source| RootfsError::Io {
        operation: "flush extracted regular file",
        path: temporary.clone(),
        source,
    })?;
    output.sync_all().map_err(|source| RootfsError::Io {
        operation: "synchronize extracted regular file",
        path: temporary.clone(),
        source,
    })?;
    drop(output);
    set_file_mode(&temporary, mode)?;

    if target.exists() {
        fs::remove_file(&target).map_err(|source| RootfsError::Io {
            operation: "replace previous rootfs file",
            path: target.clone(),
            source,
        })?;
    }
    fs::rename(&temporary, &target).map_err(|source| RootfsError::Io {
        operation: "publish extracted regular file",
        path: target,
        source,
    })?;
    Ok(())
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> Result<(), RootfsError> {
    use std::os::unix::fs::PermissionsExt as _;
    let permissions = fs::Permissions::from_mode(mode & 0o777);
    fs::set_permissions(path, permissions).map_err(|source| RootfsError::Io {
        operation: "set extracted file permissions",
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) -> Result<(), RootfsError> {
    Ok(())
}

fn create_staging_directory(parent: &Path) -> Result<PathBuf, RootfsError> {
    for _ in 0..32 {
        let sequence = ROOTFS_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".goreecloud-rootfs-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(RootfsError::Io {
                    operation: "create rootfs staging directory",
                    path,
                    source,
                });
            }
        }
    }
    Err(RootfsError::Io {
        operation: "allocate unique rootfs staging directory",
        path: parent.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique rootfs staging directory",
        ),
    })
}

fn create_temporary_file(parent: &Path) -> Result<(PathBuf, File), RootfsError> {
    for _ in 0..32 {
        let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".goreecloud-extract-{}-{sequence}",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(RootfsError::Io {
                    operation: "create extracted-file staging path",
                    path,
                    source,
                });
            }
        }
    }
    Err(RootfsError::Io {
        operation: "allocate unique extracted-file staging path",
        path: parent.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique extracted-file staging path",
        ),
    })
}

fn remove_without_following(path: &Path) -> Result<(), RootfsError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(RootfsError::Io {
                operation: "inspect whiteout target",
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if metadata.file_type().is_symlink() || metadata.is_file() {
        return fs::remove_file(path).map_err(|source| RootfsError::Io {
            operation: "remove whiteout file target",
            path: path.to_path_buf(),
            source,
        });
    }
    if !metadata.is_dir() {
        return Err(RootfsError::TargetPathNotDirectory(path.to_path_buf()));
    }

    for child in fs::read_dir(path).map_err(|source| RootfsError::Io {
        operation: "read whiteout directory target",
        path: path.to_path_buf(),
        source,
    })? {
        let child = child.map_err(|source| RootfsError::Io {
            operation: "read whiteout directory child",
            path: path.to_path_buf(),
            source,
        })?;
        remove_without_following(&child.path())?;
    }
    fs::remove_dir(path).map_err(|source| RootfsError::Io {
        operation: "remove whiteout directory target",
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tar::{Builder, Header};

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "goreecloud-containers-rootfs-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory should be created");
        path
    }

    fn tar_with_file(path: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut bytes);
            let mut header = Header::new_gnu();
            header.set_size(u64::try_from(body.len()).expect("fixture length fits in u64"));
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, body)
                .expect("fixture tar entry should append");
            builder.finish().expect("fixture tar should finish");
        }
        bytes
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(bytes)
            .expect("fixture gzip should accept data");
        encoder.finish().expect("fixture gzip should finish")
    }

    fn digest(bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::from_bytes(Sha256::digest(bytes).into())
    }

    #[test]
    fn builds_gzip_rootfs_only_after_diff_id_verification() {
        let root = temporary_directory("success");
        let tar = tar_with_file("etc/message", b"hello from GoreeCloud\n");
        let compressed = gzip(&tar);
        let layer_path = root.join("layer.tar.gz");
        fs::write(&layer_path, compressed).expect("fixture layer should be written");
        let target = root.join("rootfs");
        let layer = LayerArchive {
            media_type: OCI_LAYER_TAR_GZIP.to_owned(),
            path: layer_path,
            expected_diff_id: digest(&tar),
        };

        let built = build_rootfs(&target, &[layer], RootfsPolicy::default())
            .expect("verified fixture layer should unpack");
        assert_eq!(built.applied_layers, 1);
        assert_eq!(
            fs::read(target.join("etc/message")).expect("extracted file should be readable"),
            b"hello from GoreeCloud\n"
        );
        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn rejects_wrong_diff_id_before_publishing_rootfs() {
        let root = temporary_directory("diff-mismatch");
        let tar = tar_with_file("file", b"content");
        let layer_path = root.join("layer.tar");
        fs::write(&layer_path, &tar).expect("fixture layer should be written");
        let target = root.join("rootfs");
        let layer = LayerArchive {
            media_type: OCI_LAYER_TAR.to_owned(),
            path: layer_path,
            expected_diff_id: digest(b"wrong"),
        };

        assert!(matches!(
            build_rootfs(&target, &[layer], RootfsPolicy::default()),
            Err(RootfsError::LayerDiffIdMismatch { .. })
        ));
        assert!(!target.exists());
        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn rejects_parent_traversal_paths() {
        assert!(matches!(
            normalize_archive_path(Path::new("../escape")),
            Err(RootfsError::UnsafeArchivePath(_))
        ));
        assert!(matches!(
            normalize_archive_path(Path::new("/absolute")),
            Err(RootfsError::UnsafeArchivePath(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_traverse_symlink_parent() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("symlink-parent");
        let rootfs = root.join("staging");
        let outside = root.join("outside");
        fs::create_dir(&rootfs).expect("rootfs fixture should be created");
        fs::create_dir(&outside).expect("outside fixture should be created");
        symlink(&outside, rootfs.join("link")).expect("fixture symlink should be created");

        assert!(matches!(
            ensure_directory_chain(&rootfs, Path::new("link/child")),
            Err(RootfsError::ParentPathSymlink(_))
        ));
        assert!(!outside.join("child").exists());
        fs::remove_dir_all(root).expect("test directory should be removable");
    }
}
