use std::fmt;
use std::path::PathBuf;

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};

use proxmox_schema::{
    api, const_regex, ApiStringFormat, ApiType, ArraySchema, EnumEntry, IntegerSchema, ReturnType,
    Schema, StringSchema, Updater, UpdaterType,
};

use crate::{
    Authid, CryptMode, Fingerprint, MaintenanceMode, Userid, DATASTORE_NOTIFY_STRING_SCHEMA,
    GC_SCHEDULE_SCHEMA, PROXMOX_SAFE_ID_FORMAT, PRUNE_SCHEDULE_SCHEMA, SHA256_HEX_REGEX,
    SINGLE_LINE_COMMENT_SCHEMA, UPID,
};

const_regex! {
    pub BACKUP_NAMESPACE_REGEX = concat!(r"^", BACKUP_NS_RE!(), r"$");

    pub BACKUP_TYPE_REGEX = concat!(r"^(", BACKUP_TYPE_RE!(), r")$");

    pub BACKUP_ID_REGEX = concat!(r"^", BACKUP_ID_RE!(), r"$");

    pub BACKUP_DATE_REGEX = concat!(r"^", BACKUP_TIME_RE!() ,r"$");

    pub GROUP_PATH_REGEX = concat!(
        r"^(", BACKUP_TYPE_RE!(), ")/",
        r"(", BACKUP_ID_RE!(), r")$",
    );

    pub BACKUP_FILE_REGEX = r"^.*\.([fd]idx|blob)$";

    pub SNAPSHOT_PATH_REGEX = concat!(r"^", SNAPSHOT_PATH_REGEX_STR!(), r"$");
    pub GROUP_OR_SNAPSHOT_PATH_REGEX = concat!(r"^", GROUP_OR_SNAPSHOT_PATH_REGEX_STR!(), r"$");

    pub DATASTORE_MAP_REGEX = concat!(r"(:?", PROXMOX_SAFE_ID_REGEX_STR!(), r"=)?", PROXMOX_SAFE_ID_REGEX_STR!());
}

pub const CHUNK_DIGEST_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&SHA256_HEX_REGEX);

pub const DIR_NAME_SCHEMA: Schema = StringSchema::new("Directory name")
    .min_length(1)
    .max_length(4096)
    .schema();

pub const BACKUP_ARCHIVE_NAME_SCHEMA: Schema = StringSchema::new("Backup archive name.")
    .format(&PROXMOX_SAFE_ID_FORMAT)
    .schema();

pub const BACKUP_ID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&BACKUP_ID_REGEX);
pub const BACKUP_GROUP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&GROUP_PATH_REGEX);
pub const BACKUP_NAMESPACE_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&BACKUP_NAMESPACE_REGEX);

pub const BACKUP_ID_SCHEMA: Schema = StringSchema::new("Backup ID.")
    .format(&BACKUP_ID_FORMAT)
    .schema();

pub const BACKUP_TYPE_SCHEMA: Schema = StringSchema::new("Backup type.")
    .format(&ApiStringFormat::Enum(&[
        EnumEntry::new("vm", "Virtual Machine Backup"),
        EnumEntry::new("ct", "Container Backup"),
        EnumEntry::new("host", "Host Backup"),
    ]))
    .schema();

pub const BACKUP_TIME_SCHEMA: Schema = IntegerSchema::new("Backup time (Unix epoch.)")
    .minimum(1)
    .schema();

pub const BACKUP_GROUP_SCHEMA: Schema = StringSchema::new("Backup Group")
    .format(&BACKUP_GROUP_FORMAT)
    .schema();

/// The maximal, inclusive depth for namespaces from the root ns downwards
///
/// The datastore root name space is at depth zero (0), so we have in total eight (8) levels
pub const MAX_NAMESPACE_DEPTH: usize = 7;
pub const MAX_BACKUP_NAMESPACE_LENGTH: usize = 32 * 8; // 256
pub const BACKUP_NAMESPACE_SCHEMA: Schema = StringSchema::new("Namespace.")
    .format(&BACKUP_NAMESPACE_FORMAT)
    .max_length(MAX_BACKUP_NAMESPACE_LENGTH) // 256
    .schema();

pub const NS_MAX_DEPTH_SCHEMA: Schema =
    IntegerSchema::new("How many levels of namespaces should be operated on (0 == no recursion)")
        .minimum(0)
        .maximum(MAX_NAMESPACE_DEPTH as isize)
        .default(0)
        .schema();

pub const DATASTORE_SCHEMA: Schema = StringSchema::new("Datastore name.")
    .format(&PROXMOX_SAFE_ID_FORMAT)
    .min_length(3)
    .max_length(32)
    .schema();

pub const CHUNK_DIGEST_SCHEMA: Schema = StringSchema::new("Chunk digest (SHA256).")
    .format(&CHUNK_DIGEST_FORMAT)
    .schema();

pub const DATASTORE_MAP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&DATASTORE_MAP_REGEX);

pub const DATASTORE_MAP_SCHEMA: Schema = StringSchema::new("Datastore mapping.")
    .format(&DATASTORE_MAP_FORMAT)
    .min_length(3)
    .max_length(65)
    .type_text("(<source>=)?<target>")
    .schema();

pub const DATASTORE_MAP_ARRAY_SCHEMA: Schema =
    ArraySchema::new("Datastore mapping list.", &DATASTORE_MAP_SCHEMA).schema();

pub const DATASTORE_MAP_LIST_SCHEMA: Schema = StringSchema::new(
    "A list of Datastore mappings (or single datastore), comma separated. \
    For example 'a=b,e' maps the source datastore 'a' to target 'b and \
    all other sources to the default 'e'. If no default is given, only the \
    specified sources are mapped.",
)
.format(&ApiStringFormat::PropertyString(
    &DATASTORE_MAP_ARRAY_SCHEMA,
))
.schema();

pub const PRUNE_SCHEMA_KEEP_DAILY: Schema = IntegerSchema::new("Number of daily backups to keep.")
    .minimum(1)
    .schema();

pub const PRUNE_SCHEMA_KEEP_HOURLY: Schema =
    IntegerSchema::new("Number of hourly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_LAST: Schema = IntegerSchema::new("Number of backups to keep.")
    .minimum(1)
    .schema();

pub const PRUNE_SCHEMA_KEEP_MONTHLY: Schema =
    IntegerSchema::new("Number of monthly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_WEEKLY: Schema =
    IntegerSchema::new("Number of weekly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_YEARLY: Schema =
    IntegerSchema::new("Number of yearly backups to keep.")
        .minimum(1)
        .schema();

#[api(
    properties: {
        "keep-last": {
            schema: PRUNE_SCHEMA_KEEP_LAST,
            optional: true,
        },
        "keep-hourly": {
            schema: PRUNE_SCHEMA_KEEP_HOURLY,
            optional: true,
        },
        "keep-daily": {
            schema: PRUNE_SCHEMA_KEEP_DAILY,
            optional: true,
        },
        "keep-weekly": {
            schema: PRUNE_SCHEMA_KEEP_WEEKLY,
            optional: true,
        },
        "keep-monthly": {
            schema: PRUNE_SCHEMA_KEEP_MONTHLY,
            optional: true,
        },
        "keep-yearly": {
            schema: PRUNE_SCHEMA_KEEP_YEARLY,
            optional: true,
        },
    }
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Common pruning options
pub struct PruneOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_last: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_hourly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_daily: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_weekly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_monthly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_yearly: Option<u64>,
}

#[api]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The order to sort chunks by
pub enum ChunkOrder {
    /// Iterate chunks in the index order
    None,
    /// Iterate chunks in inode order
    Inode,
}

#[api(
    properties: {
        "chunk-order": {
            type: ChunkOrder,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Datastore tuning options
pub struct DatastoreTuning {
    /// Iterate chunks in this order
    pub chunk_order: Option<ChunkOrder>,
}

pub const DATASTORE_TUNING_STRING_SCHEMA: Schema = StringSchema::new("Datastore tuning options")
    .format(&ApiStringFormat::PropertyString(
        &DatastoreTuning::API_SCHEMA,
    ))
    .schema();

#[api(
    properties: {
        name: {
            schema: DATASTORE_SCHEMA,
        },
        path: {
            schema: DIR_NAME_SCHEMA,
        },
        "notify-user": {
            optional: true,
            type: Userid,
        },
        "notify": {
            optional: true,
            schema: DATASTORE_NOTIFY_STRING_SCHEMA,
        },
        comment: {
            optional: true,
            schema: SINGLE_LINE_COMMENT_SCHEMA,
        },
        "gc-schedule": {
            optional: true,
            schema: GC_SCHEDULE_SCHEMA,
        },
        "prune-schedule": {
            optional: true,
            schema: PRUNE_SCHEDULE_SCHEMA,
        },
        "keep-last": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_LAST,
        },
        "keep-hourly": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_HOURLY,
        },
        "keep-daily": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_DAILY,
        },
        "keep-weekly": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_WEEKLY,
        },
        "keep-monthly": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_MONTHLY,
        },
        "keep-yearly": {
            optional: true,
            schema: PRUNE_SCHEMA_KEEP_YEARLY,
        },
        "verify-new": {
            description: "If enabled, all new backups will be verified right after completion.",
            optional: true,
            type: bool,
        },
        tuning: {
            optional: true,
            schema: DATASTORE_TUNING_STRING_SCHEMA,
        },
        "maintenance-mode": {
            optional: true,
            format: &ApiStringFormat::PropertyString(&MaintenanceMode::API_SCHEMA),
            type: String,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater)]
#[serde(rename_all = "kebab-case")]
/// Datastore configuration properties.
pub struct DataStoreConfig {
    #[updater(skip)]
    pub name: String,
    #[updater(skip)]
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_last: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_hourly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_daily: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_weekly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_monthly: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_yearly: Option<u64>,
    /// If enabled, all backups will be verified right after completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_new: Option<bool>,
    /// Send job email notification to this user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_user: Option<Userid>,
    /// Send notification only for job errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    /// Datastore tuning options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuning: Option<String>,
    /// Maintenance mode, type is either 'offline' or 'read-only', message should be enclosed in "
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_mode: Option<String>,
}

impl DataStoreConfig {
    pub fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            comment: None,
            gc_schedule: None,
            prune_schedule: None,
            keep_last: None,
            keep_hourly: None,
            keep_daily: None,
            keep_weekly: None,
            keep_monthly: None,
            keep_yearly: None,
            verify_new: None,
            notify_user: None,
            notify: None,
            tuning: None,
            maintenance_mode: None,
        }
    }

    pub fn get_maintenance_mode(&self) -> Option<MaintenanceMode> {
        self.maintenance_mode
            .as_ref()
            .and_then(|str| MaintenanceMode::API_SCHEMA.parse_property_string(str).ok())
            .and_then(|value| MaintenanceMode::deserialize(value).ok())
    }
}

#[api(
    properties: {
        store: {
            schema: DATASTORE_SCHEMA,
        },
        comment: {
            optional: true,
            schema: SINGLE_LINE_COMMENT_SCHEMA,
        },
        maintenance: {
            optional: true,
            format: &ApiStringFormat::PropertyString(&MaintenanceMode::API_SCHEMA),
            type: String,
        }
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a datastore.
pub struct DataStoreListItem {
    pub store: String,
    pub comment: Option<String>,
    /// If the datastore is in maintenance mode, information about it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<String>,
}

#[api(
    properties: {
        "filename": {
            schema: BACKUP_ARCHIVE_NAME_SCHEMA,
        },
        "crypt-mode": {
            type: CryptMode,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Basic information about archive files inside a backup snapshot.
pub struct BackupContent {
    pub filename: String,
    /// Info if file is encrypted, signed, or neither.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crypt_mode: Option<CryptMode>,
    /// Archive size (from backup manifest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Result of a verify operation.
pub enum VerifyState {
    /// Verification was successful
    Ok,
    /// Verification reported one or more errors
    Failed,
}

#[api(
    properties: {
        upid: {
            type: UPID,
        },
        state: {
            type: VerifyState,
        },
    },
)]
#[derive(Serialize, Deserialize)]
/// Task properties.
pub struct SnapshotVerifyState {
    /// UPID of the verify task
    pub upid: UPID,
    /// State of the verification. Enum.
    pub state: VerifyState,
}

/// A namespace provides a logical separation between backup groups from different domains
/// (cluster, sites, ...) where uniqueness cannot be guaranteed anymore. It allows users to share a
/// datastore (i.e., one deduplication domain (chunk store)) with multiple (trusted) sites and
/// allows to form a hierarchy, for easier management and avoiding clashes between backup_ids.
///
/// NOTE: Namespaces are a logical boundary only, they do not provide a full secure separation as
/// the chunk store is still shared. So, users whom do not trust each other must not share a
/// datastore.
///
/// Implementation note: The path a namespace resolves to is always prefixed with `/ns` to avoid
/// clashes with backup group IDs and future backup_types and to have a clean separation between
/// the namespace directories and the ones from a backup snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, UpdaterType)]
pub struct BackupNamespace {
    /// The namespace subdirectories without the `ns/` intermediate directories.
    inner: Vec<String>,

    /// Cache the total length for efficiency.
    len: usize,
}

impl BackupNamespace {
    /// Returns a root namespace reference.
    pub const fn root() -> Self {
        Self {
            inner: Vec::new(),
            len: 0,
        }
    }

    /// True if this represents the root namespace.
    pub fn is_root(&self) -> bool {
        self.inner.is_empty()
    }

    /// Try to parse a string into a namespace.
    pub fn new(name: &str) -> Result<Self, Error> {
        let mut this = Self::root();

        if name.is_empty() {
            return Ok(this);
        }

        for name in name.split('/') {
            this.push(name.to_string())?;
        }
        Ok(this)
    }

    /*
    /// Try to parse a file system path (where each sub-namespace is separated by an `ns`
    /// subdirectory) into a valid namespace.
    pub fn from_path<T: AsRef<Path>>(path: T) -> Result<Self, Error> {
        use std::path::Component;

        let mut this = Self::root();
        let mut next_is_ns = true;
        for component in path.as_ref().components() {
            match component {
                Component::Normal(component) if next_is_ns => {
                    if component.to_str() != Some("ns") {
                        bail!("invalid component in path: {:?}", component);
                    }
                    next_is_ns = false;
                }
                Component::Normal(component) => {
                    this.push(
                        component
                            .to_str()
                            .ok_or_else(|| {
                                format_err!("invalid component in path: {:?}", component)
                            })?
                            .to_string(),
                    )?;
                    next_is_ns = true;
                }
                Component::RootDir => {
                    next_is_ns = true;
                }
                _ => bail!("invalid component in path: {:?}", component.as_os_str()),
            }
        }

        Ok(this)
    }
    */

    /// Try to parse a file path string (where each sub-namespace is separated by an `ns`
    /// subdirectory) into a valid namespace.
    pub fn from_path(mut path: &str) -> Result<Self, Error> {
        let mut this = Self::root();
        loop {
            match path.strip_prefix("ns/") {
                Some(next) => match next.find('/') {
                    Some(pos) => {
                        this.push(next[..pos].to_string())?;
                        path = &next[(pos + 1)..];
                    }
                    None => {
                        this.push(next.to_string())?;
                        break;
                    }
                },
                None if !path.is_empty() => {
                    bail!("invalid component in namespace path at {:?}", path);
                }
                None => break,
            }
        }
        Ok(this)
    }

    /// Create a new Namespace attached to parent
    ///
    /// `name` must be a single level namespace ID, that is, no '/' is allowed.
    /// This rule also avoids confusion about the name being a NS or NS-path
    pub fn from_parent_ns(parent: &Self, name: String) -> Result<Self, Error> {
        let mut child = parent.to_owned();
        child.push(name)?;
        Ok(child)
    }

    /// Pop one level off the namespace hierachy
    pub fn pop(&mut self) -> Option<String> {
        let dropped = self.inner.pop();
        if let Some(ref dropped) = dropped {
            self.len = self.len.saturating_sub(dropped.len() + 1);
        }
        dropped
    }

    /// Get the namespace parent as owned BackupNamespace
    pub fn parent(&self) -> Self {
        if self.is_root() {
            return Self::root();
        }

        let mut parent = self.clone();
        parent.pop();

        parent
    }

    /// Create a new namespace directly from a vec.
    ///
    /// # Safety
    ///
    /// Invalid contents may lead to inaccessible backups.
    pub unsafe fn from_vec_unchecked(components: Vec<String>) -> Self {
        let mut this = Self {
            inner: components,
            len: 0,
        };
        this.recalculate_len();
        this
    }

    /// Recalculate the length.
    fn recalculate_len(&mut self) {
        self.len = self.inner.len().max(1) - 1; // a slash between each component
        for part in &self.inner {
            self.len += part.len();
        }
    }

    /// The hierarchical depth of the namespace, 0 means top-level.
    pub fn depth(&self) -> usize {
        self.inner.len()
    }

    /// The logical name and ID of the namespace.
    pub fn name(&self) -> String {
        self.to_string()
    }

    /// The actual relative backing path of the namespace on the datastore.
    pub fn path(&self) -> PathBuf {
        self.display_as_path().to_string().into()
    }

    /// Get the current namespace length.
    ///
    /// This includes separating slashes, but does not include the `ns/` intermediate directories.
    /// This is not the *path* length, but rather the length that would be produced via
    /// `.to_string()`.
    #[inline]
    pub fn name_len(&self) -> usize {
        self.len
    }

    /// Get the current namespace path length.
    ///
    /// This includes the `ns/` subdirectory strings.
    pub fn path_len(&self) -> usize {
        self.name_len() + 3 * self.inner.len()
    }

    /// Enter a sub-namespace. Fails if nesting would become too deep or the name too long.
    pub fn push(&mut self, subdir: String) -> Result<(), Error> {
        if subdir.contains('/') {
            bail!("namespace component contained a slash");
        }

        self.push_do(subdir)
    }

    /// Assumes `subdir` already does not contain any slashes.
    /// Performs remaining checks and updates the length.
    fn push_do(&mut self, subdir: String) -> Result<(), Error> {
        if self.depth() > MAX_NAMESPACE_DEPTH {
            bail!(
                "namespace to deep, {} > max {}",
                self.inner.len(),
                MAX_NAMESPACE_DEPTH
            );
        }

        if self.len + subdir.len() + 1 > MAX_BACKUP_NAMESPACE_LENGTH {
            bail!("namespace length exceeded");
        }

        if !crate::PROXMOX_SAFE_ID_REGEX.is_match(&subdir) {
            bail!("not a valid namespace component: {subdir}");
        }

        if !self.inner.is_empty() {
            self.len += 1; // separating slash
        }
        self.len += subdir.len();
        self.inner.push(subdir);
        Ok(())
    }

    /// Return an adapter which [`Display`]s as a path with `"ns/"` prefixes in front of every
    /// component.
    pub fn display_as_path(&self) -> BackupNamespacePath {
        BackupNamespacePath(self)
    }

    /// Iterate over the subdirectories.
    pub fn components(&self) -> impl Iterator<Item = &str> + '_ {
        self.inner.iter().map(String::as_str)
    }

    /// Map NS by replacing `source_prefix` with `target_prefix`
    pub fn map_prefix(
        &self,
        source_prefix: &BackupNamespace,
        target_prefix: &BackupNamespace,
    ) -> Result<Self, Error> {
        let suffix = self
            .inner
            .strip_prefix(&source_prefix.inner[..])
            .ok_or_else(|| {
                format_err!(
                    "Failed to map namespace - {} is not a valid prefix of {}",
                    source_prefix,
                    self
                )
            })?;

        let mut new = target_prefix.clone();
        for item in suffix {
            new.push(item.clone())?;
        }
        Ok(new)
    }
}

impl fmt::Display for BackupNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;

        let mut parts = self.inner.iter();
        if let Some(first) = parts.next() {
            f.write_str(first)?;
        }
        for part in parts {
            f.write_char('/')?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

serde_plain::derive_deserialize_from_fromstr!(BackupNamespace, "valid backup namespace");

impl std::str::FromStr for BackupNamespace {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::new(name)
    }
}

serde_plain::derive_serialize_from_display!(BackupNamespace);

impl ApiType for BackupNamespace {
    const API_SCHEMA: Schema = BACKUP_NAMESPACE_SCHEMA;
}

/// Helper to format a [`BackupNamespace`] as a path component of a [`BackupGroup`].
///
/// This implements [`Display`] such that it includes the `ns/` subdirectory prefix in front of
/// every component.
pub struct BackupNamespacePath<'a>(&'a BackupNamespace);

impl fmt::Display for BackupNamespacePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut sep = "ns/";
        for part in &self.0.inner {
            f.write_str(sep)?;
            sep = "/ns/";
            f.write_str(part)?;
        }
        Ok(())
    }
}

#[api]
/// Backup types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    /// Virtual machines.
    Vm,

    /// Containers.
    Ct,

    /// "Host" backups.
    Host,
    // NOTE: if you add new types, don't forget to adapt the iter below!
}

impl BackupType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            BackupType::Vm => "vm",
            BackupType::Ct => "ct",
            BackupType::Host => "host",
        }
    }

    /// We used to have alphabetical ordering here when this was a string.
    const fn order(self) -> u8 {
        match self {
            BackupType::Ct => 0,
            BackupType::Host => 1,
            BackupType::Vm => 2,
        }
    }

    pub const fn iter() -> &'static [BackupType] {
        &[Self::Vm, Self::Ct, Self::Host]
    }
}

impl fmt::Display for BackupType {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::str::FromStr for BackupType {
    type Err = Error;

    /// Parse a backup type.
    fn from_str(ty: &str) -> Result<Self, Error> {
        Ok(match ty {
            "ct" => BackupType::Ct,
            "host" => BackupType::Host,
            "vm" => BackupType::Vm,
            _ => bail!("invalid backup type {ty:?}"),
        })
    }
}

impl std::cmp::Ord for BackupType {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order().cmp(&other.order())
    }
}

impl std::cmp::PartialOrd for BackupType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[api(
    properties: {
        "backup-type": { type: BackupType },
        "backup-id": { schema: BACKUP_ID_SCHEMA },
    },
)]
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// A backup group (without a data store).
pub struct BackupGroup {
    /// Backup type.
    #[serde(rename = "backup-type")]
    pub ty: BackupType,

    /// Backup id.
    #[serde(rename = "backup-id")]
    pub id: String,
}

impl BackupGroup {
    pub fn new<T: Into<String>>(ty: BackupType, id: T) -> Self {
        Self { ty, id: id.into() }
    }

    pub fn matches(&self, filter: &crate::GroupFilter) -> bool {
        use crate::GroupFilter;

        match filter {
            GroupFilter::Group(backup_group) => {
                match backup_group.parse::<BackupGroup>() {
                    Ok(group) => *self == group,
                    Err(_) => false, // shouldn't happen if value is schema-checked
                }
            }
            GroupFilter::BackupType(ty) => self.ty == *ty,
            GroupFilter::Regex(regex) => regex.is_match(&self.to_string()),
        }
    }
}

impl AsRef<BackupGroup> for BackupGroup {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl From<(BackupType, String)> for BackupGroup {
    #[inline]
    fn from(data: (BackupType, String)) -> Self {
        Self {
            ty: data.0,
            id: data.1,
        }
    }
}

impl std::cmp::Ord for BackupGroup {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let type_order = self.ty.cmp(&other.ty);
        if type_order != std::cmp::Ordering::Equal {
            return type_order;
        }

        // try to compare IDs numerically
        let id_self = self.id.parse::<u64>();
        let id_other = other.id.parse::<u64>();
        match (id_self, id_other) {
            (Ok(id_self), Ok(id_other)) => id_self.cmp(&id_other),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            _ => self.id.cmp(&other.id),
        }
    }
}

impl std::cmp::PartialOrd for BackupGroup {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for BackupGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.ty, self.id)
    }
}

impl std::str::FromStr for BackupGroup {
    type Err = Error;

    /// Parse a backup group.
    ///
    /// This parses strings like `vm/100".
    fn from_str(path: &str) -> Result<Self, Error> {
        let cap = GROUP_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup group path '{}'", path))?;

        Ok(Self {
            ty: cap.get(1).unwrap().as_str().parse()?,
            id: cap.get(2).unwrap().as_str().to_owned(),
        })
    }
}

#[api(
    properties: {
        "group": { type: BackupGroup },
        "backup-time": { schema: BACKUP_TIME_SCHEMA },
    },
)]
/// Uniquely identify a Backup (relative to data store)
///
/// We also call this a backup snaphost.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BackupDir {
    /// Backup group.
    #[serde(flatten)]
    pub group: BackupGroup,

    /// Backup timestamp unix epoch.
    #[serde(rename = "backup-time")]
    pub time: i64,
}

impl AsRef<BackupGroup> for BackupDir {
    #[inline]
    fn as_ref(&self) -> &BackupGroup {
        &self.group
    }
}

impl AsRef<BackupDir> for BackupDir {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl From<(BackupGroup, i64)> for BackupDir {
    fn from(data: (BackupGroup, i64)) -> Self {
        Self {
            group: data.0,
            time: data.1,
        }
    }
}

impl From<(BackupType, String, i64)> for BackupDir {
    fn from(data: (BackupType, String, i64)) -> Self {
        Self {
            group: (data.0, data.1).into(),
            time: data.2,
        }
    }
}

impl BackupDir {
    pub fn with_rfc3339<T>(ty: BackupType, id: T, backup_time_string: &str) -> Result<Self, Error>
    where
        T: Into<String>,
    {
        let time = proxmox_time::parse_rfc3339(&backup_time_string)?;
        let group = BackupGroup::new(ty, id.into());
        Ok(Self { group, time })
    }

    #[inline]
    pub fn ty(&self) -> BackupType {
        self.group.ty
    }

    #[inline]
    pub fn id(&self) -> &str {
        &self.group.id
    }
}

impl std::str::FromStr for BackupDir {
    type Err = Error;

    /// Parse a snapshot path.
    ///
    /// This parses strings like `host/elsa/2020-06-15T05:18:33Z".
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let cap = SNAPSHOT_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup snapshot path '{}'", path))?;

        BackupDir::with_rfc3339(
            cap.get(1).unwrap().as_str().parse()?,
            cap.get(2).unwrap().as_str(),
            cap.get(3).unwrap().as_str(),
        )
    }
}

impl fmt::Display for BackupDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // FIXME: log error?
        let time = proxmox_time::epoch_to_rfc3339_utc(self.time).map_err(|_| fmt::Error)?;
        write!(f, "{}/{}", self.group, time)
    }
}

/// Helper struct for places where sensible formatting of store+NS combo is required
pub struct DatastoreWithNamespace {
    pub store: String,
    pub ns: BackupNamespace,
}

impl fmt::Display for DatastoreWithNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ns.is_root() {
            write!(f, "{}", self.store)
        } else {
            write!(f, "{}/{}", self.store, self.ns)
        }
    }
}

/// Used when both a backup group or a directory can be valid.
pub enum BackupPart {
    Group(BackupGroup),
    Dir(BackupDir),
}

impl std::str::FromStr for BackupPart {
    type Err = Error;

    /// Parse a path which can be either a backup group or a snapshot dir.
    fn from_str(path: &str) -> Result<Self, Error> {
        let cap = GROUP_OR_SNAPSHOT_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup snapshot path '{}'", path))?;

        let ty = cap.get(1).unwrap().as_str().parse()?;
        let id = cap.get(2).unwrap().as_str().to_string();

        Ok(match cap.get(3) {
            Some(time) => BackupPart::Dir(BackupDir::with_rfc3339(ty, id, time.as_str())?),
            None => BackupPart::Group((ty, id).into()),
        })
    }
}

#[api(
    properties: {
        "backup": { type: BackupDir },
        comment: {
            schema: SINGLE_LINE_COMMENT_SCHEMA,
            optional: true,
        },
        verification: {
            type: SnapshotVerifyState,
            optional: true,
        },
        fingerprint: {
            type: String,
            optional: true,
        },
        files: {
            items: {
                schema: BACKUP_ARCHIVE_NAME_SCHEMA
            },
        },
        owner: {
            type: Authid,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Basic information about backup snapshot.
pub struct SnapshotListItem {
    #[serde(flatten)]
    pub backup: BackupDir,
    /// The first line from manifest "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The result of the last run verify task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<SnapshotVerifyState>,
    /// Fingerprint of encryption key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,
    /// List of contained archive files.
    pub files: Vec<BackupContent>,
    /// Overall snapshot size (sum of all archive sizes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// The owner of the snapshots group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Authid>,
    /// Protection from prunes
    #[serde(default)]
    pub protected: bool,
}

#[api(
    properties: {
        "backup": { type: BackupGroup },
        "last-backup": { schema: BACKUP_TIME_SCHEMA },
        "backup-count": {
            type: Integer,
        },
        files: {
            items: {
                schema: BACKUP_ARCHIVE_NAME_SCHEMA
            },
        },
        owner: {
            type: Authid,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a backup group.
pub struct GroupListItem {
    #[serde(flatten)]
    pub backup: BackupGroup,

    pub last_backup: i64,
    /// Number of contained snapshots
    pub backup_count: u64,
    /// List of contained archive files.
    pub files: Vec<String>,
    /// The owner of group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Authid>,
    /// The first line from group "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[api()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a backup namespace.
pub struct NamespaceListItem {
    /// A backup namespace
    pub ns: BackupNamespace,

    // TODO?
    //pub group_count: u64,
    //pub ns_count: u64,
    /// The first line from the namespace's "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[api(
    properties: {
        "backup": { type: BackupDir },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Prune result.
pub struct PruneListItem {
    #[serde(flatten)]
    pub backup: BackupDir,

    /// Keep snapshot
    pub keep: bool,
}

#[api(
    properties: {
        ct: {
            type: TypeCounts,
            optional: true,
        },
        host: {
            type: TypeCounts,
            optional: true,
        },
        vm: {
            type: TypeCounts,
            optional: true,
        },
        other: {
            type: TypeCounts,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Default)]
/// Counts of groups/snapshots per BackupType.
pub struct Counts {
    /// The counts for CT backups
    pub ct: Option<TypeCounts>,
    /// The counts for Host backups
    pub host: Option<TypeCounts>,
    /// The counts for VM backups
    pub vm: Option<TypeCounts>,
    /// The counts for other backup types
    pub other: Option<TypeCounts>,
}

#[api()]
#[derive(Serialize, Deserialize, Default)]
/// Backup Type group/snapshot counts.
pub struct TypeCounts {
    /// The number of groups of the type.
    pub groups: u64,
    /// The number of snapshots of the type.
    pub snapshots: u64,
}

#[api(
    properties: {
        "upid": {
            optional: true,
            type: UPID,
        },
    },
)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Garbage collection status.
pub struct GarbageCollectionStatus {
    pub upid: Option<String>,
    /// Number of processed index files.
    pub index_file_count: usize,
    /// Sum of bytes referred by index files.
    pub index_data_bytes: u64,
    /// Bytes used on disk.
    pub disk_bytes: u64,
    /// Chunks used on disk.
    pub disk_chunks: usize,
    /// Sum of removed bytes.
    pub removed_bytes: u64,
    /// Number of removed chunks.
    pub removed_chunks: usize,
    /// Sum of pending bytes (pending removal - kept for safety).
    pub pending_bytes: u64,
    /// Number of pending chunks (pending removal - kept for safety).
    pub pending_chunks: usize,
    /// Number of chunks marked as .bad by verify that have been removed by GC.
    pub removed_bad: usize,
    /// Number of chunks still marked as .bad after garbage collection.
    pub still_bad: usize,
}

impl Default for GarbageCollectionStatus {
    fn default() -> Self {
        GarbageCollectionStatus {
            upid: None,
            index_file_count: 0,
            index_data_bytes: 0,
            disk_bytes: 0,
            disk_chunks: 0,
            removed_bytes: 0,
            removed_chunks: 0,
            pending_bytes: 0,
            pending_chunks: 0,
            removed_bad: 0,
            still_bad: 0,
        }
    }
}

#[api(
    properties: {
        "gc-status": {
            type: GarbageCollectionStatus,
            optional: true,
        },
        counts: {
            type: Counts,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Overall Datastore status and useful information.
pub struct DataStoreStatus {
    /// Total space (bytes).
    pub total: u64,
    /// Used space (bytes).
    pub used: u64,
    /// Available space (bytes).
    pub avail: u64,
    /// Status of last GC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_status: Option<GarbageCollectionStatus>,
    /// Group/Snapshot counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
}

#[api(
    properties: {
        store: {
            schema: DATASTORE_SCHEMA,
        },
        history: {
            type: Array,
            optional: true,
            items: {
                type: Number,
                description: "The usage of a time in the past. Either null or between 0.0 and 1.0.",
            }
        },
     },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Status of a Datastore
pub struct DataStoreStatusListItem {
    pub store: String,
    /// The Size of the underlying storage in bytes. (-1 on error)
    pub total: i64,
    /// The used bytes of the underlying storage. (-1 on error)
    pub used: i64,
    /// The available bytes of the underlying storage. (-1 on error)
    pub avail: i64,
    /// A list of usages of the past (last Month).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Option<f64>>>,
    /// History start time (epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_start: Option<u64>,
    /// History resolution (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_delta: Option<u64>,
    /// Estimation of the UNIX epoch when the storage will be full.
    /// It's calculated via a simple Linear Regression (Least Squares) over the RRD data of the
    /// last Month. Missing if not enough data points are available yet. An estimate in the past
    /// means that usage is declining or not changing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_full_date: Option<i64>,
    /// An error description, for example, when the datastore could not be looked up
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Status of last GC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_status: Option<GarbageCollectionStatus>,
}

pub const ADMIN_DATASTORE_LIST_SNAPSHOTS_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of snapshots.",
        &SnapshotListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_SNAPSHOT_FILES_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of archive files inside a backup snapshots.",
        &BackupContent::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_GROUPS_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of backup groups.",
        &GroupListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_NAMESPACE_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of backup namespaces.",
        &NamespaceListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_PRUNE_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of snapshots and a flag indicating if there are kept or removed.",
        &PruneListItem::API_SCHEMA,
    )
    .schema(),
};
