use crate::rootfs::{
    LayerArchive, RootfsBuild, RootfsError, RootfsPolicy, build_rootfs,
    is_supported_layer_media_type,
};
use crate::{ContentStore, DigestParseError, ImageContentError, Sha256Digest, StoredContent};
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use url::Url;

pub const OCI_IMAGE_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
pub const DOCKER_IMAGE_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
pub const OCI_IMAGE_CONFIG: &str = "application/vnd.oci.image.config.v1+json";
pub const DOCKER_IMAGE_CONFIG: &str = "application/vnd.docker.container.image.v1+json";

const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
const DEFAULT_MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_MAX_CONFIG_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_MAX_TOKEN_BYTES: u64 = 64 * 1024;
const DEFAULT_MAX_REDIRECTS: usize = 5;
const USER_AGENT: &str = "GoreeCloud-Containers-Development/0.1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OciDescriptor {
    pub media_type: String,
    pub digest: Sha256Digest,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageManifest {
    pub media_type: String,
    pub config: OciDescriptor,
    pub layers: Vec<OciDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageProcessConfig {
    pub user: Option<String>,
    pub env: Vec<String>,
    pub entrypoint: Vec<String>,
    pub cmd: Vec<String>,
    pub working_dir: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageConfiguration {
    pub architecture: String,
    pub os: String,
    pub diff_ids: Vec<Sha256Digest>,
    pub process: ImageProcessConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulledLayer {
    pub media_type: String,
    pub digest: Sha256Digest,
    pub diff_id: Sha256Digest,
    pub size: u64,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulledImage {
    pub manifest_digest: Sha256Digest,
    pub config_digest: Sha256Digest,
    pub layers: Vec<PulledLayer>,
    pub rootfs: RootfsBuild,
    pub configuration: ImageConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryReference {
    base_url: Url,
    repository: String,
    reference: String,
}

impl RegistryReference {
    pub fn parse(
        base_url: &str,
        repository: impl Into<String>,
        reference: impl Into<String>,
    ) -> Result<Self, RegistryError> {
        let base_url = Url::parse(base_url).map_err(|source| RegistryError::InvalidUrl {
            value: base_url.to_owned(),
            source,
        })?;
        validate_network_url(&base_url)?;
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(RegistryError::CredentialsInUrl);
        }
        if base_url.query().is_some() || base_url.fragment().is_some() {
            return Err(RegistryError::InvalidRegistryBase(
                "registry base URL must not contain a query or fragment".to_owned(),
            ));
        }
        if !matches!(base_url.path(), "" | "/") {
            return Err(RegistryError::InvalidRegistryBase(
                "registry base URL must not contain a path prefix".to_owned(),
            ));
        }

        let repository = repository.into();
        validate_repository(&repository)?;
        let reference = reference.into();
        validate_reference(&reference)?;

        Ok(Self {
            base_url,
            repository,
            reference,
        })
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    fn manifest_url(&self) -> Result<Url, RegistryError> {
        build_registry_url(
            &self.base_url,
            &self.repository,
            "manifests",
            &self.reference,
        )
    }

    fn blob_url(&self, digest: Sha256Digest) -> Result<Url, RegistryError> {
        build_registry_url(
            &self.base_url,
            &self.repository,
            "blobs",
            &digest.to_string(),
        )
    }

    fn pull_scope(&self) -> String {
        format!("repository:{}:pull", self.repository)
    }
}

pub struct RegistryClient {
    agent: ureq::Agent,
    max_manifest_bytes: u64,
    max_config_bytes: u64,
    max_token_bytes: u64,
    max_redirects: usize,
}

impl RegistryClient {
    #[must_use]
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
        Self {
            agent,
            max_manifest_bytes: DEFAULT_MAX_MANIFEST_BYTES,
            max_config_bytes: DEFAULT_MAX_CONFIG_BYTES,
            max_token_bytes: DEFAULT_MAX_TOKEN_BYTES,
            max_redirects: DEFAULT_MAX_REDIRECTS,
        }
    }

    pub fn pull_image(
        &self,
        reference: &RegistryReference,
        content_store: &ContentStore,
        rootfs_target: &Path,
        rootfs_policy: RootfsPolicy,
    ) -> Result<PulledImage, RegistryError> {
        let fetched_manifest = self.fetch_manifest(reference, content_store)?;
        if !is_supported_config_media_type(&fetched_manifest.manifest.config.media_type) {
            return Err(RegistryError::UnsupportedConfigMediaType(
                fetched_manifest.manifest.config.media_type.clone(),
            ));
        }
        if fetched_manifest.manifest.config.size > self.max_config_bytes {
            return Err(RegistryError::BodyTooLarge {
                context: "image configuration",
                maximum: self.max_config_bytes,
            });
        }

        let config_content =
            self.fetch_blob(reference, &fetched_manifest.manifest.config, content_store)?;
        let config_bytes = read_file_bounded(
            &config_content.path,
            self.max_config_bytes,
            "image configuration",
        )?;
        let configuration = parse_image_configuration(&config_bytes)?;
        if configuration.os != "linux" {
            return Err(RegistryError::UnsupportedImageOperatingSystem(
                configuration.os.clone(),
            ));
        }
        if configuration.diff_ids.len() != fetched_manifest.manifest.layers.len() {
            return Err(RegistryError::LayerDiffIdCountMismatch {
                layers: fetched_manifest.manifest.layers.len(),
                diff_ids: configuration.diff_ids.len(),
            });
        }

        let mut pulled_layers = Vec::with_capacity(fetched_manifest.manifest.layers.len());
        let mut archives = Vec::with_capacity(fetched_manifest.manifest.layers.len());
        for (descriptor, diff_id) in fetched_manifest
            .manifest
            .layers
            .iter()
            .zip(configuration.diff_ids.iter().copied())
        {
            if !is_supported_layer_media_type(&descriptor.media_type) {
                return Err(RegistryError::UnsupportedLayerMediaType(
                    descriptor.media_type.clone(),
                ));
            }
            let content = self.fetch_blob(reference, descriptor, content_store)?;
            archives.push(LayerArchive {
                media_type: descriptor.media_type.clone(),
                path: content.path.clone(),
                expected_diff_id: diff_id,
            });
            pulled_layers.push(PulledLayer {
                media_type: descriptor.media_type.clone(),
                digest: descriptor.digest,
                diff_id,
                size: descriptor.size,
                path: content.path,
            });
        }

        let rootfs = build_rootfs(rootfs_target, &archives, rootfs_policy)?;
        Ok(PulledImage {
            manifest_digest: fetched_manifest.digest,
            config_digest: fetched_manifest.manifest.config.digest,
            layers: pulled_layers,
            rootfs,
            configuration,
        })
    }

    fn fetch_manifest(
        &self,
        reference: &RegistryReference,
        content_store: &ContentStore,
    ) -> Result<FetchedManifest, RegistryError> {
        let request_url = reference.manifest_url()?;
        let response =
            self.authorized_get(request_url, Some(MANIFEST_ACCEPT), &reference.pull_scope())?;
        if response.status() != 200 {
            return Err(RegistryError::UnexpectedStatus {
                context: "manifest retrieval",
                status: response.status(),
            });
        }

        let content_type = response
            .header("Content-Type")
            .map(normalize_media_type)
            .ok_or(RegistryError::MissingHeader("Content-Type"))?;
        if !is_supported_manifest_media_type(&content_type) {
            return Err(RegistryError::UnsupportedManifestMediaType(content_type));
        }

        let header_digest = response.header("Docker-Content-Digest").map(str::to_owned);
        let body = read_response_bounded(response, self.max_manifest_bytes, "manifest")?;

        let reference_digest = Sha256Digest::from_str(reference.reference()).ok();
        let header_digest = match header_digest {
            Some(value) => Some(parse_digest(&value)?),
            None => None,
        };
        let expected = match (reference_digest, header_digest) {
            (Some(reference_digest), Some(header_digest)) if reference_digest != header_digest => {
                return Err(RegistryError::ConflictingManifestDigest {
                    reference: reference_digest,
                    header: header_digest,
                });
            }
            (Some(reference_digest), _) => reference_digest,
            (None, Some(header_digest)) => header_digest,
            (None, None) => return Err(RegistryError::MissingHeader("Docker-Content-Digest")),
        };

        content_store.ingest_reader(expected, body.as_slice())?;
        let manifest = parse_manifest(&body, &content_type)?;
        Ok(FetchedManifest {
            digest: expected,
            manifest,
        })
    }

    fn fetch_blob(
        &self,
        reference: &RegistryReference,
        descriptor: &OciDescriptor,
        content_store: &ContentStore,
    ) -> Result<StoredContent, RegistryError> {
        if descriptor.size > content_store.max_content_bytes() {
            return Err(RegistryError::BodyTooLarge {
                context: "blob descriptor",
                maximum: content_store.max_content_bytes(),
            });
        }

        let request_url = reference.blob_url(descriptor.digest)?;
        let response = self.authorized_get(request_url, None, &reference.pull_scope())?;
        if response.status() != 200 {
            return Err(RegistryError::UnexpectedStatus {
                context: "blob retrieval",
                status: response.status(),
            });
        }
        if let Some(content_length) = response.header("Content-Length") {
            let content_length =
                content_length
                    .parse::<u64>()
                    .map_err(|_| RegistryError::InvalidHeader {
                        name: "Content-Length",
                        value: content_length.to_owned(),
                    })?;
            if content_length > content_store.max_content_bytes() {
                return Err(RegistryError::BodyTooLarge {
                    context: "blob response",
                    maximum: content_store.max_content_bytes(),
                });
            }
        }

        let stored = content_store.ingest_reader(descriptor.digest, response.into_reader())?;
        if stored.size != descriptor.size {
            return Err(RegistryError::DescriptorSizeMismatch {
                digest: descriptor.digest,
                expected: descriptor.size,
                actual: stored.size,
            });
        }
        Ok(stored)
    }

    fn authorized_get(
        &self,
        url: Url,
        accept: Option<&str>,
        scope: &str,
    ) -> Result<ureq::Response, RegistryError> {
        let (_, first) = self.get_following_redirects(url.clone(), accept, None)?;
        if first.status() != 401 {
            return Ok(first);
        }

        let challenge = first
            .header("WWW-Authenticate")
            .ok_or(RegistryError::MissingHeader("WWW-Authenticate"))?;
        let challenge = parse_bearer_challenge(challenge)?;
        let token = self.fetch_bearer_token(&challenge, scope)?;
        let authorization = format!("Bearer {token}");
        let (_, second) = self.get_following_redirects(url, accept, Some(&authorization))?;
        if second.status() == 401 {
            return Err(RegistryError::AuthenticationRejected);
        }
        Ok(second)
    }

    fn fetch_bearer_token(
        &self,
        challenge: &BearerChallenge,
        requested_scope: &str,
    ) -> Result<String, RegistryError> {
        let mut url = Url::parse(&challenge.realm).map_err(|source| RegistryError::InvalidUrl {
            value: challenge.realm.clone(),
            source,
        })?;
        validate_network_url(&url)?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(RegistryError::CredentialsInUrl);
        }
        let scope = validated_bearer_scope(challenge, requested_scope)?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(service) = &challenge.service {
                query.append_pair("service", service);
            }
            query.append_pair("scope", scope);
        }

        let (_, response) = self.get_following_redirects(url, Some("application/json"), None)?;
        if response.status() != 200 {
            return Err(RegistryError::UnexpectedStatus {
                context: "registry bearer-token retrieval",
                status: response.status(),
            });
        }
        let body = read_response_bounded(response, self.max_token_bytes, "bearer token")?;
        let parsed: RawTokenResponse =
            serde_json::from_slice(&body).map_err(|source| RegistryError::Json {
                context: "registry bearer-token response",
                source,
            })?;
        let token = parsed.token.or(parsed.access_token).ok_or(
            RegistryError::InvalidAuthenticationChallenge(
                "token response did not contain token or access_token".to_owned(),
            ),
        )?;
        if token.is_empty()
            || token.len() > usize::try_from(self.max_token_bytes).unwrap_or(usize::MAX)
        {
            return Err(RegistryError::InvalidAuthenticationChallenge(
                "registry bearer token is empty or exceeds the configured bound".to_owned(),
            ));
        }
        Ok(token)
    }

    fn get_following_redirects(
        &self,
        initial_url: Url,
        accept: Option<&str>,
        authorization: Option<&str>,
    ) -> Result<(Url, ureq::Response), RegistryError> {
        validate_network_url(&initial_url)?;
        let mut current = initial_url;
        let mut authorization = authorization.map(str::to_owned);

        for redirects in 0..=self.max_redirects {
            let response = self.call_get(&current, accept, authorization.as_deref())?;
            if !is_redirect(response.status()) {
                return Ok((current, response));
            }
            if redirects == self.max_redirects {
                return Err(RegistryError::TooManyRedirects {
                    maximum: self.max_redirects,
                });
            }
            let location = response
                .header("Location")
                .ok_or(RegistryError::MissingHeader("Location"))?;
            let next = current
                .join(location)
                .map_err(|source| RegistryError::InvalidUrl {
                    value: location.to_owned(),
                    source,
                })?;
            validate_network_url(&next)?;
            if current.scheme() == "https" && next.scheme() != "https" {
                return Err(RegistryError::InsecureRedirect {
                    from: redact_url(&current),
                    to: redact_url(&next),
                });
            }
            if !same_origin(&current, &next) {
                authorization = None;
            }
            current = next;
        }
        unreachable!("redirect loop always returns or errors")
    }

    fn call_get(
        &self,
        url: &Url,
        accept: Option<&str>,
        authorization: Option<&str>,
    ) -> Result<ureq::Response, RegistryError> {
        let mut request = self.agent.get(url.as_str()).set("User-Agent", USER_AGENT);
        if let Some(accept) = accept {
            request = request.set("Accept", accept);
        }
        if let Some(authorization) = authorization {
            request = request.set("Authorization", authorization);
        }
        match request.call() {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(_, response)) => Ok(response),
            Err(error) => Err(RegistryError::HttpTransport {
                url: redact_url(url),
                detail: error.to_string(),
            }),
        }
    }
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum RegistryError {
    InvalidUrl {
        value: String,
        source: url::ParseError,
    },
    InvalidRegistryBase(String),
    InsecureUrl(String),
    CredentialsInUrl,
    InvalidRepository(String),
    InvalidReference(String),
    MissingHeader(&'static str),
    InvalidHeader {
        name: &'static str,
        value: String,
    },
    UnexpectedStatus {
        context: &'static str,
        status: u16,
    },
    HttpTransport {
        url: String,
        detail: String,
    },
    BodyTooLarge {
        context: &'static str,
        maximum: u64,
    },
    InvalidAuthenticationChallenge(String),
    AuthenticationRejected,
    TooManyRedirects {
        maximum: usize,
    },
    InsecureRedirect {
        from: String,
        to: String,
    },
    UnsupportedManifestMediaType(String),
    ManifestMediaTypeMismatch {
        response: String,
        document: String,
    },
    UnsupportedConfigMediaType(String),
    UnsupportedLayerMediaType(String),
    UnsupportedImageOperatingSystem(String),
    InvalidManifestSchemaVersion(u32),
    InvalidRootfsType(String),
    LayerDiffIdCountMismatch {
        layers: usize,
        diff_ids: usize,
    },
    ConflictingManifestDigest {
        reference: Sha256Digest,
        header: Sha256Digest,
    },
    DescriptorSizeMismatch {
        digest: Sha256Digest,
        expected: u64,
        actual: u64,
    },
    Digest {
        value: String,
        source: DigestParseError,
    },
    Json {
        context: &'static str,
        source: serde_json::Error,
    },
    Content(ImageContentError),
    Rootfs(RootfsError),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl { value, source } => write!(formatter, "invalid URL '{value}': {source}"),
            Self::InvalidRegistryBase(message) => formatter.write_str(message),
            Self::InsecureUrl(url) => write!(
                formatter,
                "registry network URL must use HTTPS; plain HTTP is accepted only for loopback Development fixtures: {url}"
            ),
            Self::CredentialsInUrl => formatter.write_str(
                "credentials embedded in registry or authentication URLs are not accepted",
            ),
            Self::InvalidRepository(value) => write!(
                formatter,
                "invalid registry repository '{value}'; lowercase slash-separated components are required"
            ),
            Self::InvalidReference(value) => write!(
                formatter,
                "invalid image reference '{value}'; expected a valid tag or lowercase sha256 digest"
            ),
            Self::MissingHeader(name) => write!(formatter, "required registry response header is missing: {name}"),
            Self::InvalidHeader { name, value } => {
                write!(formatter, "invalid registry response header {name}: {value}")
            }
            Self::UnexpectedStatus { context, status } => {
                write!(formatter, "{context} returned unexpected HTTP status {status}")
            }
            Self::HttpTransport { url, detail } => {
                write!(formatter, "registry HTTP request to '{url}' failed: {detail}")
            }
            Self::BodyTooLarge { context, maximum } => {
                write!(formatter, "{context} exceeds configured limit of {maximum} bytes")
            }
            Self::InvalidAuthenticationChallenge(message) => {
                write!(formatter, "invalid registry authentication challenge: {message}")
            }
            Self::AuthenticationRejected => formatter.write_str(
                "registry rejected the anonymous bearer-token retry; credentialed registry authentication is not implemented in this Development slice",
            ),
            Self::TooManyRedirects { maximum } => write!(
                formatter,
                "registry request exceeded configured redirect limit of {maximum}"
            ),
            Self::InsecureRedirect { from, to } => write!(
                formatter,
                "registry redirect would downgrade HTTPS: '{from}' -> '{to}'"
            ),
            Self::UnsupportedManifestMediaType(value) => {
                write!(formatter, "unsupported image manifest media type: {value}")
            }
            Self::ManifestMediaTypeMismatch { response, document } => write!(
                formatter,
                "manifest media type does not match HTTP Content-Type: response '{response}', document '{document}'"
            ),
            Self::UnsupportedConfigMediaType(value) => {
                write!(formatter, "unsupported image configuration media type: {value}")
            }
            Self::UnsupportedLayerMediaType(value) => {
                write!(formatter, "unsupported image layer media type: {value}")
            }
            Self::UnsupportedImageOperatingSystem(value) => write!(
                formatter,
                "unsupported image operating system '{value}'; this Development engine currently builds Linux root filesystems only"
            ),
            Self::InvalidManifestSchemaVersion(value) => {
                write!(formatter, "unsupported image manifest schemaVersion {value}; expected 2")
            }
            Self::InvalidRootfsType(value) => {
                write!(formatter, "unsupported image configuration rootfs type '{value}'")
            }
            Self::LayerDiffIdCountMismatch { layers, diff_ids } => write!(
                formatter,
                "image configuration contains {diff_ids} diff IDs for {layers} manifest layers"
            ),
            Self::ConflictingManifestDigest { reference, header } => write!(
                formatter,
                "manifest digest reference {reference} conflicts with Docker-Content-Digest {header}"
            ),
            Self::DescriptorSizeMismatch {
                digest,
                expected,
                actual,
            } => write!(
                formatter,
                "descriptor size mismatch for {digest}: expected {expected} bytes, received {actual}"
            ),
            Self::Digest { value, source } => {
                write!(formatter, "invalid OCI digest '{value}': {source}")
            }
            Self::Json { context, source } => {
                write!(formatter, "failed to parse {context} JSON: {source}")
            }
            Self::Content(error) => error.fmt(formatter),
            Self::Rootfs(error) => error.fmt(formatter),
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

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUrl { source, .. } => Some(source),
            Self::Digest { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Content(error) => Some(error),
            Self::Rootfs(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<ImageContentError> for RegistryError {
    fn from(error: ImageContentError) -> Self {
        Self::Content(error)
    }
}

impl From<RootfsError> for RegistryError {
    fn from(error: RootfsError) -> Self {
        Self::Rootfs(error)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDescriptor {
    media_type: String,
    digest: String,
    size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    schema_version: u32,
    media_type: Option<String>,
    config: RawDescriptor,
    layers: Vec<RawDescriptor>,
}

#[derive(Deserialize)]
struct RawRootfs {
    #[serde(rename = "type")]
    kind: String,
    diff_ids: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawProcessConfig {
    user: Option<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    entrypoint: Vec<String>,
    #[serde(default)]
    cmd: Vec<String>,
    working_dir: Option<String>,
}

#[derive(Deserialize)]
struct RawImageConfiguration {
    architecture: String,
    os: String,
    rootfs: RawRootfs,
    #[serde(default)]
    config: RawProcessConfig,
}

#[derive(Deserialize)]
struct RawTokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

struct FetchedManifest {
    digest: Sha256Digest,
    manifest: ImageManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BearerChallenge {
    realm: String,
    service: Option<String>,
    scope: Option<String>,
}

fn parse_manifest(body: &[u8], response_media_type: &str) -> Result<ImageManifest, RegistryError> {
    let raw: RawManifest = serde_json::from_slice(body).map_err(|source| RegistryError::Json {
        context: "image manifest",
        source,
    })?;
    if raw.schema_version != 2 {
        return Err(RegistryError::InvalidManifestSchemaVersion(
            raw.schema_version,
        ));
    }
    if let Some(document_media_type) = raw.media_type {
        let document_media_type = normalize_media_type(&document_media_type);
        if !is_supported_manifest_media_type(&document_media_type) {
            return Err(RegistryError::UnsupportedManifestMediaType(
                document_media_type,
            ));
        }
        if document_media_type != response_media_type {
            return Err(RegistryError::ManifestMediaTypeMismatch {
                response: response_media_type.to_owned(),
                document: document_media_type,
            });
        }
    }

    Ok(ImageManifest {
        media_type: response_media_type.to_owned(),
        config: convert_descriptor(raw.config)?,
        layers: raw
            .layers
            .into_iter()
            .map(convert_descriptor)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_image_configuration(body: &[u8]) -> Result<ImageConfiguration, RegistryError> {
    let raw: RawImageConfiguration =
        serde_json::from_slice(body).map_err(|source| RegistryError::Json {
            context: "image configuration",
            source,
        })?;
    if raw.rootfs.kind != "layers" {
        return Err(RegistryError::InvalidRootfsType(raw.rootfs.kind));
    }
    let diff_ids = raw
        .rootfs
        .diff_ids
        .iter()
        .map(|value| parse_digest(value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ImageConfiguration {
        architecture: raw.architecture,
        os: raw.os,
        diff_ids,
        process: ImageProcessConfig {
            user: raw.config.user,
            env: raw.config.env,
            entrypoint: raw.config.entrypoint,
            cmd: raw.config.cmd,
            working_dir: raw.config.working_dir,
        },
    })
}

fn convert_descriptor(raw: RawDescriptor) -> Result<OciDescriptor, RegistryError> {
    Ok(OciDescriptor {
        media_type: normalize_media_type(&raw.media_type),
        digest: parse_digest(&raw.digest)?,
        size: raw.size,
    })
}

fn parse_digest(value: &str) -> Result<Sha256Digest, RegistryError> {
    value.parse().map_err(|source| RegistryError::Digest {
        value: value.to_owned(),
        source,
    })
}

fn is_supported_manifest_media_type(media_type: &str) -> bool {
    matches!(media_type, OCI_IMAGE_MANIFEST | DOCKER_IMAGE_MANIFEST)
}

fn is_supported_config_media_type(media_type: &str) -> bool {
    matches!(media_type, OCI_IMAGE_CONFIG | DOCKER_IMAGE_CONFIG)
}

fn normalize_media_type(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}

fn validate_repository(repository: &str) -> Result<(), RegistryError> {
    if repository.is_empty() || repository.len() > 255 {
        return Err(RegistryError::InvalidRepository(repository.to_owned()));
    }
    for component in repository.split('/') {
        if component.is_empty() {
            return Err(RegistryError::InvalidRepository(repository.to_owned()));
        }
        let bytes = component.as_bytes();
        if !bytes
            .first()
            .is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
            || !bytes
                .last()
                .is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
            || !bytes.iter().all(|value| {
                value.is_ascii_lowercase()
                    || value.is_ascii_digit()
                    || matches!(value, b'.' | b'_' | b'-')
            })
        {
            return Err(RegistryError::InvalidRepository(repository.to_owned()));
        }
    }
    Ok(())
}

fn validate_reference(reference: &str) -> Result<(), RegistryError> {
    if Sha256Digest::from_str(reference).is_ok() {
        return Ok(());
    }
    if reference.starts_with("sha256:") {
        return Err(RegistryError::InvalidReference(reference.to_owned()));
    }
    let bytes = reference.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'_' | b'.' | b'-'))
    {
        return Err(RegistryError::InvalidReference(reference.to_owned()));
    }
    Ok(())
}

fn build_registry_url(
    base: &Url,
    repository: &str,
    category: &str,
    value: &str,
) -> Result<Url, RegistryError> {
    let mut url = base.clone();
    {
        let mut segments = url.path_segments_mut().map_err(|()| {
            RegistryError::InvalidRegistryBase(
                "registry base URL cannot be hierarchical".to_owned(),
            )
        })?;
        segments.clear();
        segments.push("v2");
        for component in repository.split('/') {
            segments.push(component);
        }
        segments.push(category);
        segments.push(value);
    }
    Ok(url)
}

fn validate_network_url(url: &Url) -> Result<(), RegistryError> {
    if url.scheme() == "https" {
        return Ok(());
    }
    if url.scheme() == "http" && url.host_str().is_some_and(is_loopback_host) {
        return Ok(());
    }
    Err(RegistryError::InsecureUrl(redact_url(url)))
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || IpAddr::from_str(host).is_ok_and(|address| address.is_loopback())
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn redact_url(url: &Url) -> String {
    let mut redacted = url.clone();
    redacted.set_query(None);
    redacted.set_fragment(None);
    redacted.to_string()
}

fn read_response_bounded(
    response: ureq::Response,
    maximum: u64,
    context: &'static str,
) -> Result<Vec<u8>, RegistryError> {
    if let Some(content_length) = response.header("Content-Length") {
        let parsed = content_length
            .parse::<u64>()
            .map_err(|_| RegistryError::InvalidHeader {
                name: "Content-Length",
                value: content_length.to_owned(),
            })?;
        if parsed > maximum {
            return Err(RegistryError::BodyTooLarge { context, maximum });
        }
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| RegistryError::Io {
            operation: "read bounded registry response",
            path: PathBuf::from("<registry-response>"),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(RegistryError::BodyTooLarge { context, maximum });
    }
    Ok(bytes)
}

fn read_file_bounded(
    path: &Path,
    maximum: u64,
    context: &'static str,
) -> Result<Vec<u8>, RegistryError> {
    let file = File::open(path).map_err(|source| RegistryError::Io {
        operation: "open stored image metadata",
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| RegistryError::Io {
            operation: "read stored image metadata",
            path: path.to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(RegistryError::BodyTooLarge { context, maximum });
    }
    Ok(bytes)
}

fn parse_bearer_challenge(value: &str) -> Result<BearerChallenge, RegistryError> {
    let (scheme, parameters) = value.split_once(' ').ok_or_else(|| {
        RegistryError::InvalidAuthenticationChallenge(
            "expected Bearer authentication scheme and parameters".to_owned(),
        )
    })?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(RegistryError::InvalidAuthenticationChallenge(format!(
            "unsupported authentication scheme '{scheme}'; only anonymous Bearer-token flow is implemented"
        )));
    }

    let mut realm = None;
    let mut service = None;
    let mut scope = None;
    for parameter in split_auth_parameters(parameters)? {
        let (name, raw_value) = parameter.split_once('=').ok_or_else(|| {
            RegistryError::InvalidAuthenticationChallenge(
                "authentication parameter is missing '='".to_owned(),
            )
        })?;
        let decoded = decode_auth_value(raw_value.trim())?;
        match name.trim().to_ascii_lowercase().as_str() {
            "realm" => realm = Some(decoded),
            "service" => service = Some(decoded),
            "scope" => scope = Some(decoded),
            _ => {}
        }
    }
    let realm = realm.ok_or_else(|| {
        RegistryError::InvalidAuthenticationChallenge(
            "Bearer authentication challenge is missing realm".to_owned(),
        )
    })?;
    Ok(BearerChallenge {
        realm,
        service,
        scope,
    })
}

fn validated_bearer_scope<'a>(
    challenge: &BearerChallenge,
    requested_scope: &'a str,
) -> Result<&'a str, RegistryError> {
    if let Some(advertised_scope) = challenge.scope.as_deref() {
        if advertised_scope != requested_scope {
            return Err(RegistryError::InvalidAuthenticationChallenge(format!(
                "registry requested bearer scope '{advertised_scope}', but GoreeCloud requested only '{requested_scope}'"
            )));
        }
    }
    Ok(requested_scope)
}

fn split_auth_parameters(value: &str) -> Result<Vec<&str>, RegistryError> {
    let bytes = value.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match *byte {
            b'\\' if quoted => escaped = true,
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || escaped {
        return Err(RegistryError::InvalidAuthenticationChallenge(
            "unterminated quoted authentication parameter".to_owned(),
        ));
    }
    parts.push(value[start..].trim());
    if parts.iter().any(|part| part.is_empty()) {
        return Err(RegistryError::InvalidAuthenticationChallenge(
            "empty authentication parameter".to_owned(),
        ));
    }
    Ok(parts)
}

fn decode_auth_value(value: &str) -> Result<String, RegistryError> {
    if !value.starts_with('"') {
        return Ok(value.to_owned());
    }
    if value.len() < 2 || !value.ends_with('"') {
        return Err(RegistryError::InvalidAuthenticationChallenge(
            "unterminated quoted authentication value".to_owned(),
        ));
    }
    let inner = &value[1..value.len() - 1];
    let mut decoded = String::new();
    let mut escaped = false;
    for character in inner.chars() {
        if escaped {
            decoded.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            decoded.push(character);
        }
    }
    if escaped {
        return Err(RegistryError::InvalidAuthenticationChallenge(
            "unterminated authentication escape".to_owned(),
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rootfs::OCI_LAYER_TAR_GZIP;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::thread::JoinHandle;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tar::Header;

    #[derive(Clone)]
    struct FixtureResponse {
        status: u16,
        content_type: Option<&'static str>,
        digest: Option<String>,
        body: Vec<u8>,
    }

    struct FixtureRegistry {
        base_url: String,
        handle: Option<JoinHandle<()>>,
    }

    impl FixtureRegistry {
        fn start(responses: BTreeMap<String, FixtureResponse>, requests: usize) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("fixture registry should bind");
            let address = listener
                .local_addr()
                .expect("fixture registry should have an address");
            let handle = thread::spawn(move || {
                for _ in 0..requests {
                    let (stream, _) = listener.accept().expect("fixture request should arrive");
                    serve_fixture_request(stream, &responses);
                }
            });
            Self {
                base_url: format!("http://{address}"),
                handle: Some(handle),
            }
        }

        fn join(mut self) {
            if let Some(handle) = self.handle.take() {
                handle
                    .join()
                    .expect("fixture registry thread should finish");
            }
        }
    }

    fn serve_fixture_request(mut stream: TcpStream, responses: &BTreeMap<String, FixtureResponse>) {
        let path = {
            let mut reader = BufReader::new(&mut stream);
            let mut first = String::new();
            reader
                .read_line(&mut first)
                .expect("fixture request line should be readable");
            let path = first
                .split_whitespace()
                .nth(1)
                .expect("fixture request should contain a path")
                .to_owned();
            loop {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .expect("fixture request headers should be readable");
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            path
        };

        let response = responses.get(&path).cloned().unwrap_or(FixtureResponse {
            status: 404,
            content_type: Some("text/plain"),
            digest: None,
            body: b"not found".to_vec(),
        });
        let reason = if response.status == 200 {
            "OK"
        } else {
            "Not Found"
        };
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            reason,
            response.body.len()
        )
        .expect("fixture response headers should write");
        if let Some(content_type) = response.content_type {
            write!(stream, "Content-Type: {content_type}\r\n")
                .expect("fixture content type should write");
        }
        if let Some(digest) = response.digest {
            write!(stream, "Docker-Content-Digest: {digest}\r\n")
                .expect("fixture digest should write");
        }
        write!(stream, "\r\n").expect("fixture header terminator should write");
        stream
            .write_all(&response.body)
            .expect("fixture response body should write");
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "goreecloud-containers-registry-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("test directory should be created");
        path
    }

    fn digest(bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::from_bytes(Sha256::digest(bytes).into())
    }

    fn tar_with_file(path: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut bytes);
            let mut header = Header::new_gnu();
            header.set_size(u64::try_from(body.len()).expect("fixture length fits in u64"));
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, path, body)
                .expect("fixture tar should accept a file");
            builder.finish().expect("fixture tar should finish");
        }
        bytes
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(bytes)
            .expect("fixture gzip should accept bytes");
        encoder.finish().expect("fixture gzip should finish")
    }

    #[test]
    fn validates_registry_repository_and_reference() {
        assert!(RegistryReference::parse("https://registry.example", "team/app", "v1.2").is_ok());
        assert!(RegistryReference::parse("http://registry.example", "team/app", "v1").is_err());
        assert!(RegistryReference::parse("https://registry.example", "Team/App", "v1").is_err());
        assert!(
            RegistryReference::parse("https://registry.example", "team/app", "bad tag").is_err()
        );
    }

    #[test]
    fn parses_anonymous_bearer_challenge() {
        let challenge = parse_bearer_challenge(
            "Bearer realm=\"https://auth.example/token\",service=\"registry.example\",scope=\"repository:team/app:pull\"",
        )
        .expect("valid Bearer challenge should parse");
        assert_eq!(challenge.realm, "https://auth.example/token");
        assert_eq!(challenge.service.as_deref(), Some("registry.example"));
        assert_eq!(challenge.scope.as_deref(), Some("repository:team/app:pull"));
        assert_eq!(
            validated_bearer_scope(&challenge, "repository:team/app:pull")
                .expect("matching scope should be accepted"),
            "repository:team/app:pull"
        );
    }

    #[test]
    fn rejects_bearer_scope_escalation() {
        let challenge = BearerChallenge {
            realm: "https://auth.example/token".to_owned(),
            service: Some("registry.example".to_owned()),
            scope: Some("repository:team/admin:push,pull".to_owned()),
        };
        assert!(matches!(
            validated_bearer_scope(&challenge, "repository:team/app:pull"),
            Err(RegistryError::InvalidAuthenticationChallenge(_))
        ));
    }

    #[test]
    fn pulls_verified_manifest_config_and_layer_into_rootfs() {
        let root = temporary_directory("pull");
        let store_root = root.join("store");
        std::fs::create_dir(&store_root).expect("content store should be created");
        let rootfs = root.join("rootfs");

        let tar = tar_with_file("bin/demo", b"hello from GoreeCloud\n");
        let compressed_layer = gzip(&tar);
        let layer_digest = digest(&compressed_layer);
        let diff_id = digest(&tar);
        let config = format!(
            "{{\"architecture\":\"amd64\",\"os\":\"linux\",\"rootfs\":{{\"type\":\"layers\",\"diff_ids\":[\"{diff_id}\"]}},\"config\":{{\"Cmd\":[\"/bin/demo\"],\"WorkingDir\":\"/\"}}}}"
        )
        .into_bytes();
        let config_digest = digest(&config);
        let manifest = format!(
            "{{\"schemaVersion\":2,\"mediaType\":\"{OCI_IMAGE_MANIFEST}\",\"config\":{{\"mediaType\":\"{OCI_IMAGE_CONFIG}\",\"digest\":\"{config_digest}\",\"size\":{}}},\"layers\":[{{\"mediaType\":\"{OCI_LAYER_TAR_GZIP}\",\"digest\":\"{layer_digest}\",\"size\":{}}}]}}",
            config.len(),
            compressed_layer.len()
        )
        .into_bytes();
        let manifest_digest = digest(&manifest);

        let mut responses = BTreeMap::new();
        responses.insert(
            "/v2/demo/example/manifests/latest".to_owned(),
            FixtureResponse {
                status: 200,
                content_type: Some(OCI_IMAGE_MANIFEST),
                digest: Some(manifest_digest.to_string()),
                body: manifest,
            },
        );
        responses.insert(
            format!("/v2/demo/example/blobs/{config_digest}"),
            FixtureResponse {
                status: 200,
                content_type: Some("application/octet-stream"),
                digest: None,
                body: config,
            },
        );
        responses.insert(
            format!("/v2/demo/example/blobs/{layer_digest}"),
            FixtureResponse {
                status: 200,
                content_type: Some("application/octet-stream"),
                digest: None,
                body: compressed_layer,
            },
        );
        let fixture = FixtureRegistry::start(responses, 3);
        let reference = RegistryReference::parse(&fixture.base_url, "demo/example", "latest")
            .expect("loopback fixture registry should be accepted");
        let store = ContentStore::open(&store_root, 32 * 1024 * 1024)
            .expect("fixture content store should open");

        let pulled = RegistryClient::new()
            .pull_image(&reference, &store, &rootfs, RootfsPolicy::default())
            .expect("fixture image should pull and unpack");
        assert_eq!(pulled.manifest_digest, manifest_digest);
        assert_eq!(pulled.config_digest, config_digest);
        assert_eq!(pulled.layers.len(), 1);
        assert_eq!(pulled.layers[0].digest, layer_digest);
        assert_eq!(
            std::fs::read(rootfs.join("bin/demo")).expect("pulled rootfs file should be readable"),
            b"hello from GoreeCloud\n"
        );

        fixture.join();
        std::fs::remove_dir_all(root).expect("test directory should be removable");
    }

    #[test]
    fn rejects_blob_digest_mismatch_before_rootfs_publication() {
        let root = temporary_directory("digest-mismatch");
        let store_root = root.join("store");
        std::fs::create_dir(&store_root).expect("content store should be created");
        let rootfs = root.join("rootfs");
        let real_config = b"{\"architecture\":\"amd64\"}".to_vec();
        let expected_config_digest = digest(b"different configuration");
        let manifest = format!(
            "{{\"schemaVersion\":2,\"mediaType\":\"{OCI_IMAGE_MANIFEST}\",\"config\":{{\"mediaType\":\"{OCI_IMAGE_CONFIG}\",\"digest\":\"{expected_config_digest}\",\"size\":{}}},\"layers\":[]}}",
            real_config.len()
        )
        .into_bytes();
        let manifest_digest = digest(&manifest);

        let mut responses = BTreeMap::new();
        responses.insert(
            "/v2/demo/example/manifests/latest".to_owned(),
            FixtureResponse {
                status: 200,
                content_type: Some(OCI_IMAGE_MANIFEST),
                digest: Some(manifest_digest.to_string()),
                body: manifest,
            },
        );
        responses.insert(
            format!("/v2/demo/example/blobs/{expected_config_digest}"),
            FixtureResponse {
                status: 200,
                content_type: Some("application/octet-stream"),
                digest: None,
                body: real_config,
            },
        );
        let fixture = FixtureRegistry::start(responses, 2);
        let reference = RegistryReference::parse(&fixture.base_url, "demo/example", "latest")
            .expect("loopback fixture registry should be accepted");
        let store = ContentStore::open(&store_root, 32 * 1024 * 1024)
            .expect("fixture content store should open");

        assert!(matches!(
            RegistryClient::new().pull_image(&reference, &store, &rootfs, RootfsPolicy::default()),
            Err(RegistryError::Content(
                ImageContentError::DigestMismatch { .. }
            ))
        ));
        assert!(!rootfs.exists());

        fixture.join();
        std::fs::remove_dir_all(root).expect("test directory should be removable");
    }
}
