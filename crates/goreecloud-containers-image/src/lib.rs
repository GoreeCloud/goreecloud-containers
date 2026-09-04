use sha2::{Digest as _, Sha256};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

pub const DEFAULT_MAX_CONTENT_BYTES: u64 = 512 * 1024 * 1024;
const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;
static INCOMING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(SHA256_PREFIX)?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Sha256Digest {
    type Err = DigestParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hex = value
            .strip_prefix(SHA256_PREFIX)
            .ok_or(DigestParseError::UnsupportedAlgorithm)?;
        if hex.len() != SHA256_HEX_LENGTH {
            return Err(DigestParseError::InvalidLength {
                actual: hex.len(),
                expected: SHA256_HEX_LENGTH,
            });
        }

        let mut bytes = [0_u8; 32];
        let raw = hex.as_bytes();
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = decode_hex(raw[index * 2]).ok_or(DigestParseError::InvalidHex {
                index: index * 2,
            })?;
            let low = decode_hex(raw[index * 2 + 1]).ok_or(DigestParseError::InvalidHex {
                index: index * 2 + 1,
            })?;
            *byte = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

fn decode_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestParseError {
    UnsupportedAlgorithm,
    InvalidLength { actual: usize, expected: usize },
    InvalidHex { index: usize },
}

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedAlgorithm => {
                formatter.write_str("unsupported digest; expected lowercase sha256:<64-hex>")
            }
            Self::InvalidLength { actual, expected } => write!(
                formatter,
                "invalid sha256 digest length {actual}; expected {expected} hexadecimal characters"
            ),
            Self::InvalidHex { index } => write!(
                formatter,
                "invalid sha256 digest character at hexadecimal index {index}; lowercase hexadecimal is required"
            ),
        }
    }
}

impl Error for DigestParseError {}

#[derive(Debug)]
pub enum ImageContentError {
    StoreRootNotAbsolute(PathBuf),
    StoreRootSymlink(PathBuf),
    StoreRootNotDirectory(PathBuf),
    SourcePathNotAbsolute(PathBuf),
    SourcePathSymlink(PathBuf),
    SourceNotRegularFile(PathBuf),
    SubdirectorySymlink(PathBuf),
    SubdirectoryNotDirectory(PathBuf),
    InstalledContentSymlink(PathBuf),
    InstalledContentNotRegularFile(PathBuf),
    InvalidMaxContentBytes,
    ContentTooLarge { maximum: u64 },
    DigestMismatch {
        expected: Sha256Digest,
        actual: Sha256Digest,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ImageContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreRootNotAbsolute(path) => write!(
                formatter,
                "content-store root must be absolute: {}",
                path.display()
            ),
            Self::StoreRootSymlink(path) => write!(
                formatter,
                "content-store root must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::StoreRootNotDirectory(path) => write!(
                formatter,
                "content-store root is not a directory: {}",
                path.display()
            ),
            Self::SourcePathNotAbsolute(path) => {
                write!(formatter, "source path must be absolute: {}", path.display())
            }
            Self::SourcePathSymlink(path) => write!(
                formatter,
                "source path must not itself be a symbolic link: {}",
                path.display()
            ),
            Self::SourceNotRegularFile(path) => {
                write!(formatter, "source is not a regular file: {}", path.display())
            }
            Self::SubdirectorySymlink(path) => write!(
                formatter,
                "content-store subdirectory must not be a symbolic link: {}",
                path.display()
            ),
            Self::SubdirectoryNotDirectory(path) => write!(
                formatter,
                "content-store subdirectory is not a directory: {}",
                path.display()
            ),
            Self::InstalledContentSymlink(path) => write!(
                formatter,
                "installed content path must not be a symbolic link: {}",
                path.display()
            ),
            Self::InstalledContentNotRegularFile(path) => write!(
                formatter,
                "installed content path is not a regular file: {}",
                path.display()
            ),
            Self::InvalidMaxContentBytes => {
                formatter.write_str("maximum content size must be greater than zero")
            }
            Self::ContentTooLarge { maximum } => {
                write!(formatter, "content exceeds configured limit of {maximum} bytes")
            }
            Self::DigestMismatch { expected, actual } => {
                write!(formatter, "digest mismatch: expected {expected}, calculated {actual}")
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

impl Error for ImageContentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredContent {
    pub digest: Sha256Digest,
    pub path: PathBuf,
    pub size: u64,
    pub reused_existing: bool,
}

#[derive(Clone, Debug)]
pub struct ContentStore {
    root: PathBuf,
    max_content_bytes: u64,
}

impl ContentStore {
    pub fn open(
        root: impl Into<PathBuf>,
        max_content_bytes: u64,
    ) -> Result<Self, ImageContentError> {
        if max_content_bytes == 0 {
            return Err(ImageContentError::InvalidMaxContentBytes);
        }

        let root = root.into();
        if !root.is_absolute() {
            return Err(ImageContentError::StoreRootNotAbsolute(root));
        }
        let metadata = fs::symlink_metadata(&root).map_err(|source| ImageContentError::Io {
            operation: "inspect content-store root",
            path: root.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ImageContentError::StoreRootSymlink(root));
        }
        if !metadata.is_dir() {
            return Err(ImageContentError::StoreRootNotDirectory(root));
        }
        let root = fs::canonicalize(&root).map_err(|source| ImageContentError::Io {
            operation: "canonicalize content-store root",
            path: root,
            source,
        })?;

        Ok(Self {
            root,
            max_content_bytes,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn max_content_bytes(&self) -> u64 {
        self.max_content_bytes
    }

    pub fn ingest_file(
        &self,
        expected: Sha256Digest,
        source_path: &Path,
    ) -> Result<StoredContent, ImageContentError> {
        if !source_path.is_absolute() {
            return Err(ImageContentError::SourcePathNotAbsolute(
                source_path.to_path_buf(),
            ));
        }
        let metadata =
            fs::symlink_metadata(source_path).map_err(|source| ImageContentError::Io {
                operation: "inspect source content",
                path: source_path.to_path_buf(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(ImageContentError::SourcePathSymlink(
                source_path.to_path_buf(),
            ));
        }
        if !metadata.is_file() {
            return Err(ImageContentError::SourceNotRegularFile(
                source_path.to_path_buf(),
            ));
        }

        let file = File::open(source_path).map_err(|source| ImageContentError::Io {
            operation: "open source content",
            path: source_path.to_path_buf(),
            source,
        })?;
        self.ingest_reader(expected, file)
    }

    pub fn ingest_reader(
        &self,
        expected: Sha256Digest,
        mut reader: impl Read,
    ) -> Result<StoredContent, ImageContentError> {
        let algorithm_directory = self.ensure_subdirectory("sha256")?;
        let target_path = algorithm_directory.join(expected_hex(expected));
        if target_path.exists() {
            let size = verify_installed_content(
                &target_path,
                expected,
                self.max_content_bytes,
            )?;
            return Ok(StoredContent {
                digest: expected,
                path: target_path,
                size,
                reused_existing: true,
            });
        }

        let incoming_directory = self.ensure_subdirectory(".incoming")?;
        let (incoming_path, mut incoming_file) =
            create_incoming_file(&incoming_directory).map_err(|(path, source)| {
                ImageContentError::Io {
                    operation: "create incoming content file",
                    path,
                    source,
                }
            })?;

        let result = copy_and_hash(&mut reader, &mut incoming_file, self.max_content_bytes);
        let (size, actual) = match result {
            Ok(value) => value,
            Err(error) => {
                let _ = fs::remove_file(&incoming_path);
                return Err(error);
            }
        };

        if actual != expected {
            let _ = fs::remove_file(&incoming_path);
            return Err(ImageContentError::DigestMismatch { expected, actual });
        }

        incoming_file
            .sync_all()
            .map_err(|source| ImageContentError::Io {
                operation: "synchronize incoming content",
                path: incoming_path.clone(),
                source,
            })?;
        drop(incoming_file);

        match fs::hard_link(&incoming_path, &target_path) {
            Ok(()) => {
                fs::remove_file(&incoming_path).map_err(|source| ImageContentError::Io {
                    operation: "remove installed incoming content",
                    path: incoming_path,
                    source,
                })?;
                Ok(StoredContent {
                    digest: expected,
                    path: target_path,
                    size,
                    reused_existing: false,
                })
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&incoming_path);
                let existing_size =
                    verify_installed_content(&target_path, expected, self.max_content_bytes)?;
                Ok(StoredContent {
                    digest: expected,
                    path: target_path,
                    size: existing_size,
                    reused_existing: true,
                })
            }
            Err(source) => {
                let _ = fs::remove_file(&incoming_path);
                Err(ImageContentError::Io {
                    operation: "install verified content",
                    path: target_path,
                    source,
                })
            }
        }
    }

    fn ensure_subdirectory(&self, name: &str) -> Result<PathBuf, ImageContentError> {
        let path = self.root.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(ImageContentError::Io {
                    operation: "create content-store subdirectory",
                    path,
                    source,
                });
            }
        }

        let metadata = fs::symlink_metadata(&path).map_err(|source| ImageContentError::Io {
            operation: "inspect content-store subdirectory",
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ImageContentError::SubdirectorySymlink(path));
        }
        if !metadata.is_dir() {
            return Err(ImageContentError::SubdirectoryNotDirectory(path));
        }
        Ok(path)
    }
}

fn expected_hex(digest: Sha256Digest) -> String {
    let rendered = digest.to_string();
    rendered[SHA256_PREFIX.len()..].to_owned()
}

fn copy_and_hash(
    reader: &mut impl Read,
    writer: &mut impl Write,
    maximum: u64,
) -> Result<(u64, Sha256Digest), ImageContentError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let count = reader.read(&mut buffer).map_err(|source| ImageContentError::Io {
            operation: "read source content",
            path: PathBuf::from("<reader>"),
            source,
        })?;
        if count == 0 {
            break;
        }

        let count_u64 = u64::try_from(count).expect("read buffer length always fits in u64");
        total = total
            .checked_add(count_u64)
            .ok_or(ImageContentError::ContentTooLarge { maximum })?;
        if total > maximum {
            return Err(ImageContentError::ContentTooLarge { maximum });
        }

        hasher.update(&buffer[..count]);
        writer
            .write_all(&buffer[..count])
            .map_err(|source| ImageContentError::Io {
                operation: "write incoming content",
                path: PathBuf::from("<incoming>"),
                source,
            })?;
    }

    let actual = Sha256Digest::from_bytes(hasher.finalize().into());
    Ok((total, actual))
}

fn verify_installed_content(
    path: &Path,
    expected: Sha256Digest,
    maximum: u64,
) -> Result<u64, ImageContentError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ImageContentError::Io {
        operation: "inspect installed content",
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ImageContentError::InstalledContentSymlink(
            path.to_path_buf(),
        ));
    }
    if !metadata.is_file() {
        return Err(ImageContentError::InstalledContentNotRegularFile(
            path.to_path_buf(),
        ));
    }

    let mut file = File::open(path).map_err(|source| ImageContentError::Io {
        operation: "open installed content",
        path: path.to_path_buf(),
        source,
    })?;
    let mut sink = io::sink();
    let (size, actual) = copy_and_hash(&mut file, &mut sink, maximum)?;
    if actual != expected {
        return Err(ImageContentError::DigestMismatch { expected, actual });
    }
    Ok(size)
}

fn create_incoming_file(directory: &Path) -> Result<(PathBuf, File), (PathBuf, io::Error)> {
    for _ in 0..32 {
        let sequence = INCOMING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(".content-{}-{sequence}", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err((path, source)),
        }
    }

    let path = directory.join(".content-exhausted");
    Err((
        path,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique incoming content path",
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const HELLO_DIGEST: &str =
        "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "goreecloud-containers-image-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory should be created");
        path
    }

    fn hello_digest() -> Sha256Digest {
        HELLO_DIGEST
            .parse()
            .expect("known sha256 digest should parse")
    }

    #[test]
    fn parses_and_renders_lowercase_sha256_digest() {
        let digest = hello_digest();
        assert_eq!(digest.to_string(), HELLO_DIGEST);
    }

    #[test]
    fn rejects_unsupported_or_malformed_digests() {
        assert!(matches!(
            "sha512:abcd".parse::<Sha256Digest>(),
            Err(DigestParseError::UnsupportedAlgorithm)
        ));
        assert!(matches!(
            "sha256:abcd".parse::<Sha256Digest>(),
            Err(DigestParseError::InvalidLength { .. })
        ));
        let invalid = format!("sha256:{}G", "0".repeat(63));
        assert!(matches!(
            invalid.parse::<Sha256Digest>(),
            Err(DigestParseError::InvalidHex { .. })
        ));
    }

    #[test]
    fn ingests_verified_content_and_reuses_existing_blob() {
        let root = temporary_directory("ingest");
        let source = root.join("source");
        fs::write(&source, b"hello").expect("fixture should be written");
        let store_root = root.join("store");
        fs::create_dir(&store_root).expect("store directory should be created");
        let store =
            ContentStore::open(&store_root, 1024).expect("absolute test store should open");

        let installed = store
            .ingest_file(hello_digest(), &source)
            .expect("verified content should install");
        assert_eq!(installed.size, 5);
        assert!(!installed.reused_existing);
        assert_eq!(
            fs::read(&installed.path).expect("installed content should be readable"),
            b"hello"
        );

        let second = store
            .ingest_file(hello_digest(), &source)
            .expect("existing verified content should be reusable");
        assert!(second.reused_existing);
        assert_eq!(second.path, installed.path);

        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn rejects_digest_mismatch_without_installing_target() {
        let root = temporary_directory("mismatch");
        let store_root = root.join("store");
        fs::create_dir(&store_root).expect("store directory should be created");
        let store =
            ContentStore::open(&store_root, 1024).expect("absolute test store should open");
        let error = store.ingest_reader(hello_digest(), b"different".as_slice());
        assert!(matches!(
            error,
            Err(ImageContentError::DigestMismatch { .. })
        ));
        let target = store_root
            .join("sha256")
            .join(&HELLO_DIGEST[SHA256_PREFIX.len()..]);
        assert!(!target.exists());

        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn enforces_content_size_limit() {
        let root = temporary_directory("limit");
        let store_root = root.join("store");
        fs::create_dir(&store_root).expect("store directory should be created");
        let store =
            ContentStore::open(&store_root, 4).expect("absolute test store should open");
        let error = store.ingest_reader(hello_digest(), b"hello".as_slice());
        assert!(matches!(
            error,
            Err(ImageContentError::ContentTooLarge { maximum: 4 })
        ));

        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn rejects_relative_store_root_and_source_path() {
        assert!(matches!(
            ContentStore::open("relative/store", 1024),
            Err(ImageContentError::StoreRootNotAbsolute(_))
        ));

        let root = temporary_directory("relative-source");
        let store =
            ContentStore::open(&root, 1024).expect("absolute test store should open");
        assert!(matches!(
            store.ingest_file(hello_digest(), Path::new("relative-source")),
            Err(ImageContentError::SourcePathNotAbsolute(_))
        ));
        fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_store_root() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("symlink");
        let actual = root.join("actual");
        fs::create_dir(&actual).expect("actual store should be created");
        let link = root.join("link");
        symlink(&actual, &link).expect("test symlink should be created");

        assert!(matches!(
            ContentStore::open(&link, 1024),
            Err(ImageContentError::StoreRootSymlink(_))
        ));

        fs::remove_dir_all(root).expect("test directory should be removable");
    }
}
