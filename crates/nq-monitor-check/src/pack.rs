//! Typed, fail-closed check-pack discovery and selection.
//!
//! A package being compiled only makes it *available*. A pack is executable
//! only after an explicit [`PackSelection`] names the pack and the checks to
//! enable, and its pack-specific configuration has validated.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::{type_name, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

pub const CHECK_PACK_CONTRACT_VERSION: &str = "nq.monitor.check_pack.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PackId(String);

impl PackId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RegistryError> {
        let value = value.into();
        validate_id("pack ID", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for PackId {
    type Err = RegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for PackId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CheckId(String);

impl CheckId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RegistryError> {
        let value = value.into();
        validate_id("check ID", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for CheckId {
    type Err = RegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for CheckId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

fn validate_id(kind: &str, value: &str) -> Result<(), RegistryError> {
    let valid = !value.is_empty()
        && value == value.trim()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase);
    if valid {
        Ok(())
    } else {
        Err(RegistryError::InvalidId {
            kind: kind.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckCost {
    Cheap,
    Moderate,
    Expensive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckLocality {
    Local,
    LocalHelper,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckPrivilege {
    Unprivileged,
    OptionalElevatedHelper,
    RequiredElevatedHelper,
    SecretBearing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckDescriptor {
    pub check_id: CheckId,
    pub title: String,
    pub cost: CheckCost,
    pub locality: CheckLocality,
    pub privilege: CheckPrivilege,
    pub observation_schema: String,
    pub operator_claim: String,
    #[serde(default)]
    pub unknowns: Vec<String>,
    #[serde(default)]
    pub remediation_hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackDescriptor {
    pub pack_id: PackId,
    pub contract_version: String,
    pub title: String,
    /// Deployment eligibility only. Registration never enables a pack.
    pub default_policy: PackDefaultPolicy,
    pub checks: Vec<CheckDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackDefaultPolicy {
    /// May be selected by an explicitly documented minimal-public
    /// configuration, but is still never enabled by registration alone.
    MinimalPublicCandidate,
    /// Must be named explicitly in deployment configuration.
    ExplicitOnly,
}

/// A typed pack definition.
///
/// The registry erases only configuration validation. Packs with a direct
/// collector additionally implement [`ExecutableCheckPack`]; composition
/// packs may remain definitions until a composition root supplies the generic
/// acquisition primitives they select.
pub trait CheckPackDefinition: Send + Sync + 'static {
    type Config: DeserializeOwned + Send + Sync + 'static;

    fn descriptor() -> PackDescriptor;

    fn validate_config(
        config: &Self::Config,
        enabled_checks: &BTreeSet<CheckId>,
    ) -> Result<(), PackConfigError>;
}

/// A pack with an independently executable collector.
///
/// The associated output keeps each family typed; the contract does not mint
/// a universal event or evidence payload. `collect` is the low-level
/// implementation hook: calling it directly bypasses registry selection and
/// validation. Deployment composition must execute through a resolved
/// [`EnabledPack`].
pub trait ExecutableCheckPack: CheckPackDefinition {
    type Observation;

    fn collect(config: &Self::Config, enabled_checks: &BTreeSet<CheckId>) -> Self::Observation;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSelection {
    /// Empty means no packs are enabled. Availability is never activation.
    #[serde(default)]
    pub enabled: Vec<PackSelectionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSelectionEntry {
    pub pack_id: PackId,
    /// Checks are always explicit. An empty list is rejected.
    pub checks: Vec<CheckId>,
    #[serde(default = "empty_object")]
    pub config: Value,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

/// One selection that passed the validator registered for this exact pack
/// implementation.
///
/// Identity, enabled checks, raw configuration, and implementation binding are
/// intentionally opaque. Callers can inspect immutable identities but cannot
/// rewrite a resolved selection after validation.
///
/// ```compile_fail
/// use nq_monitor_check::EnabledPack;
///
/// fn mutate_after_validation(pack: &mut EnabledPack) {
///     pack.checks.clear();
/// }
/// ```
///
/// ```compile_fail
/// use nq_monitor_check::{EnabledPack, PackId};
///
/// fn substitute_identity(pack: &mut EnabledPack) {
///     pack.pack_id = PackId::parse("different.pack").unwrap();
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EnabledPack {
    pack_id: PackId,
    checks: BTreeSet<CheckId>,
    config: Value,
    implementation_type: TypeId,
    implementation_name: &'static str,
}

impl EnabledPack {
    pub fn pack_id(&self) -> &PackId {
        &self.pack_id
    }

    pub fn checks(&self) -> &BTreeSet<CheckId> {
        &self.checks
    }

    pub fn parse_config<P>(&self) -> Result<P::Config, PackConfigError>
    where
        P: CheckPackDefinition,
    {
        self.require_registered_implementation::<P>()?;
        let expected = P::descriptor().pack_id;
        if self.pack_id != expected {
            return Err(PackConfigError::new(
                "pack_id",
                format!(
                    "configuration belongs to `{}`, not requested pack `{expected}`",
                    self.pack_id
                ),
            ));
        }
        decode_config::<P>(&self.config, &self.checks)
    }

    pub fn collect<P>(&self) -> Result<P::Observation, PackConfigError>
    where
        P: ExecutableCheckPack,
    {
        let config = self.parse_config::<P>()?;
        Ok(P::collect(&config, &self.checks))
    }

    fn require_registered_implementation<P>(&self) -> Result<(), PackConfigError>
    where
        P: CheckPackDefinition,
    {
        if self.implementation_type == TypeId::of::<P>() {
            Ok(())
        } else {
            Err(PackConfigError::new(
                "implementation",
                format!(
                    "resolved pack `{}` is bound to `{}` and cannot be reinterpreted as `{}`",
                    self.pack_id,
                    self.implementation_name,
                    type_name::<P>()
                ),
            ))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedPacks {
    enabled: BTreeMap<PackId, EnabledPack>,
}

impl ResolvedPacks {
    pub fn is_enabled(&self, pack_id: &str) -> bool {
        self.enabled
            .keys()
            .any(|candidate| candidate.as_str() == pack_id)
    }

    pub fn enabled(&self) -> impl Iterator<Item = &EnabledPack> {
        self.enabled.values()
    }

    pub fn get(&self, pack_id: &str) -> Option<&EnabledPack> {
        self.enabled
            .iter()
            .find_map(|(candidate, pack)| (candidate.as_str() == pack_id).then_some(pack))
    }
}

type ValidateFn = fn(&Value, &BTreeSet<CheckId>) -> Result<(), PackConfigError>;

struct Registration {
    descriptor: PackDescriptor,
    validate: ValidateFn,
    implementation_type: TypeId,
    implementation_name: &'static str,
}

#[derive(Default)]
pub struct CheckPackRegistry {
    available: BTreeMap<PackId, Registration>,
}

impl CheckPackRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P>(&mut self) -> Result<(), RegistryError>
    where
        P: CheckPackDefinition,
    {
        let descriptor = P::descriptor();
        validate_descriptor(&descriptor)?;
        if self.available.contains_key(&descriptor.pack_id) {
            return Err(RegistryError::DuplicatePack {
                pack_id: descriptor.pack_id,
            });
        }
        self.available.insert(
            descriptor.pack_id.clone(),
            Registration {
                descriptor,
                validate: decode_and_discard::<P>,
                implementation_type: TypeId::of::<P>(),
                implementation_name: type_name::<P>(),
            },
        );
        Ok(())
    }

    pub fn available(&self) -> impl Iterator<Item = &PackDescriptor> {
        self.available.values().map(|entry| &entry.descriptor)
    }

    pub fn resolve(&self, selection: PackSelection) -> Result<ResolvedPacks, RegistryError> {
        let mut enabled = BTreeMap::new();
        for entry in selection.enabled {
            let registration =
                self.available
                    .get(&entry.pack_id)
                    .ok_or_else(|| RegistryError::UnknownPack {
                        pack_id: entry.pack_id.clone(),
                        available: self.available.keys().cloned().collect(),
                    })?;
            if enabled.contains_key(&entry.pack_id) {
                return Err(RegistryError::DuplicateSelection {
                    pack_id: entry.pack_id,
                });
            }
            if entry.checks.is_empty() {
                return Err(RegistryError::NoChecksEnabled {
                    pack_id: entry.pack_id,
                });
            }
            let check_count = entry.checks.len();
            let checks: BTreeSet<_> = entry.checks.into_iter().collect();
            if checks.len() != check_count {
                return Err(RegistryError::DuplicateCheckSelection {
                    pack_id: entry.pack_id,
                });
            }
            let known: BTreeSet<_> = registration
                .descriptor
                .checks
                .iter()
                .map(|check| check.check_id.clone())
                .collect();
            for check_id in &checks {
                if !known.contains(check_id) {
                    return Err(RegistryError::UnknownCheck {
                        pack_id: entry.pack_id,
                        check_id: check_id.clone(),
                        available: known.into_iter().collect(),
                    });
                }
            }
            (registration.validate)(&entry.config, &checks).map_err(|source| {
                RegistryError::InvalidConfig {
                    pack_id: entry.pack_id.clone(),
                    source,
                }
            })?;
            let pack = EnabledPack {
                pack_id: entry.pack_id.clone(),
                checks,
                config: entry.config,
                implementation_type: registration.implementation_type,
                implementation_name: registration.implementation_name,
            };
            enabled.insert(entry.pack_id, pack);
        }
        Ok(ResolvedPacks { enabled })
    }
}

fn decode_and_discard<P>(
    value: &Value,
    enabled_checks: &BTreeSet<CheckId>,
) -> Result<(), PackConfigError>
where
    P: CheckPackDefinition,
{
    decode_config::<P>(value, enabled_checks).map(drop)
}

fn decode_config<P>(
    value: &Value,
    enabled_checks: &BTreeSet<CheckId>,
) -> Result<P::Config, PackConfigError>
where
    P: CheckPackDefinition,
{
    let config = serde_json::from_value(value.clone()).map_err(|source| {
        PackConfigError::new("config", format!("invalid pack configuration: {source}"))
    })?;
    P::validate_config(&config, enabled_checks)?;
    Ok(config)
}

fn validate_descriptor(descriptor: &PackDescriptor) -> Result<(), RegistryError> {
    if descriptor.contract_version != CHECK_PACK_CONTRACT_VERSION {
        return Err(RegistryError::UnsupportedContract {
            pack_id: descriptor.pack_id.clone(),
            expected: CHECK_PACK_CONTRACT_VERSION.to_string(),
            actual: descriptor.contract_version.clone(),
        });
    }
    if descriptor.title.trim().is_empty() {
        return Err(RegistryError::InvalidDescriptor {
            pack_id: descriptor.pack_id.clone(),
            message: "title must not be empty".to_string(),
        });
    }
    if descriptor.checks.is_empty() {
        return Err(RegistryError::InvalidDescriptor {
            pack_id: descriptor.pack_id.clone(),
            message: "must declare at least one check".to_string(),
        });
    }
    let mut ids = BTreeSet::new();
    for check in &descriptor.checks {
        if !ids.insert(check.check_id.clone()) {
            return Err(RegistryError::InvalidDescriptor {
                pack_id: descriptor.pack_id.clone(),
                message: format!("duplicate check ID `{}`", check.check_id),
            });
        }
        for (field, value) in [
            ("title", &check.title),
            ("observation_schema", &check.observation_schema),
            ("operator_claim", &check.operator_claim),
        ] {
            if value.trim().is_empty() {
                return Err(RegistryError::InvalidDescriptor {
                    pack_id: descriptor.pack_id.clone(),
                    message: format!("check `{}` {field} must not be empty", check.check_id),
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackConfigError {
    pub field: String,
    pub message: String,
}

impl PackConfigError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for PackConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid pack configuration field `{}`: {}",
            self.field, self.message
        )
    }
}

impl std::error::Error for PackConfigError {}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid {kind} `{value}`; use lowercase letters, digits, `.`, `_`, or `-`, beginning with a letter")]
    InvalidId { kind: String, value: String },
    #[error("pack `{pack_id}` is registered more than once")]
    DuplicatePack { pack_id: PackId },
    #[error("pack `{pack_id}` is selected more than once")]
    DuplicateSelection { pack_id: PackId },
    #[error("pack `{pack_id}` enables no checks; list at least one check ID explicitly")]
    NoChecksEnabled { pack_id: PackId },
    #[error("pack `{pack_id}` lists a check more than once")]
    DuplicateCheckSelection { pack_id: PackId },
    #[error("unknown pack `{pack_id}`; available packs: {available:?}")]
    UnknownPack {
        pack_id: PackId,
        available: Vec<PackId>,
    },
    #[error("unknown check `{check_id}` for pack `{pack_id}`; available checks: {available:?}")]
    UnknownCheck {
        pack_id: PackId,
        check_id: CheckId,
        available: Vec<CheckId>,
    },
    #[error("pack `{pack_id}` uses unsupported contract `{actual}`; expected `{expected}`")]
    UnsupportedContract {
        pack_id: PackId,
        expected: String,
        actual: String,
    },
    #[error("invalid descriptor for pack `{pack_id}`: {message}")]
    InvalidDescriptor { pack_id: PackId, message: String },
    #[error("invalid configuration for pack `{pack_id}`: {source}")]
    InvalidConfig {
        pack_id: PackId,
        #[source]
        source: PackConfigError,
    },
}

/// Marker used only to keep generic type parameters visible to rustdoc when a
/// composition root stores typed pack adapters.
#[doc(hidden)]
pub struct PackType<P: CheckPackDefinition>(PhantomData<P>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static RUNS: AtomicUsize = AtomicUsize::new(0);
    static SUBSTITUTION_RUNS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureConfig {
        required_path: String,
    }

    struct FixturePack;

    impl CheckPackDefinition for FixturePack {
        type Config = FixtureConfig;

        fn descriptor() -> PackDescriptor {
            PackDescriptor {
                pack_id: PackId::parse("fixture.host").unwrap(),
                contract_version: CHECK_PACK_CONTRACT_VERSION.to_string(),
                title: "Fixture host pack".to_string(),
                default_policy: PackDefaultPolicy::ExplicitOnly,
                checks: vec![CheckDescriptor {
                    check_id: CheckId::parse("host.resources").unwrap(),
                    title: "Host resources".to_string(),
                    cost: CheckCost::Cheap,
                    locality: CheckLocality::Local,
                    privilege: CheckPrivilege::Unprivileged,
                    observation_schema: "fixture.host.resources.v1".to_string(),
                    operator_claim: "Host resources were observed".to_string(),
                    unknowns: vec!["Application impact is not established".to_string()],
                    remediation_hints: vec!["Inspect the constrained resource".to_string()],
                }],
            }
        }

        fn validate_config(
            config: &Self::Config,
            _enabled_checks: &BTreeSet<CheckId>,
        ) -> Result<(), PackConfigError> {
            if config.required_path.starts_with('/') {
                Ok(())
            } else {
                Err(PackConfigError::new("required_path", "must be absolute"))
            }
        }
    }

    impl ExecutableCheckPack for FixturePack {
        type Observation = usize;

        fn collect(
            _config: &Self::Config,
            _enabled_checks: &BTreeSet<CheckId>,
        ) -> Self::Observation {
            RUNS.fetch_add(1, Ordering::SeqCst);
            RUNS.load(Ordering::SeqCst)
        }
    }

    /// Deliberately advertises the same mutable pack ID as `FixturePack`.
    /// Matching descriptor text is not authority to reinterpret a token that
    /// the registry resolved against another Rust implementation.
    struct SubstitutionPack;

    impl CheckPackDefinition for SubstitutionPack {
        type Config = FixtureConfig;

        fn descriptor() -> PackDescriptor {
            FixturePack::descriptor()
        }

        fn validate_config(
            _config: &Self::Config,
            _enabled_checks: &BTreeSet<CheckId>,
        ) -> Result<(), PackConfigError> {
            Ok(())
        }
    }

    impl ExecutableCheckPack for SubstitutionPack {
        type Observation = usize;

        fn collect(
            _config: &Self::Config,
            _enabled_checks: &BTreeSet<CheckId>,
        ) -> Self::Observation {
            SUBSTITUTION_RUNS.fetch_add(1, Ordering::SeqCst);
            SUBSTITUTION_RUNS.load(Ordering::SeqCst)
        }
    }

    fn registry() -> CheckPackRegistry {
        let mut registry = CheckPackRegistry::new();
        registry.register::<FixturePack>().unwrap();
        registry
    }

    #[test]
    fn registration_makes_pack_available_but_not_enabled_or_executed() {
        RUNS.store(0, Ordering::SeqCst);
        let registry = registry();
        assert_eq!(registry.available().count(), 1);
        let resolved = registry.resolve(PackSelection::default()).unwrap();
        assert!(!resolved.is_enabled("fixture.host"));
        assert_eq!(RUNS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unknown_pack_and_check_fail_closed() {
        let unknown_pack: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.missing",
                "checks": ["host.resources"],
                "config": {"required_path": "/proc"}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(unknown_pack),
            Err(RegistryError::UnknownPack { .. })
        ));

        let unknown_check: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.typo"],
                "config": {"required_path": "/proc"}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(unknown_check),
            Err(RegistryError::UnknownCheck { .. })
        ));
    }

    #[test]
    fn duplicate_pack_and_check_selections_fail_closed() {
        let duplicate_check: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.resources", "host.resources"],
                "config": {"required_path": "/proc"}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(duplicate_check),
            Err(RegistryError::DuplicateCheckSelection { .. })
        ));

        let duplicate_pack: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [
                {
                    "pack_id": "fixture.host",
                    "checks": ["host.resources"],
                    "config": {"required_path": "/proc"}
                },
                {
                    "pack_id": "fixture.host",
                    "checks": ["host.resources"],
                    "config": {"required_path": "/proc"}
                }
            ]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(duplicate_pack),
            Err(RegistryError::DuplicateSelection { .. })
        ));
    }

    #[test]
    fn unknown_config_fields_and_invalid_values_fail_closed() {
        let unknown_field: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.resources"],
                "config": {"required_path": "/proc", "typo": true}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(unknown_field),
            Err(RegistryError::InvalidConfig { .. })
        ));

        let invalid_value: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.resources"],
                "config": {"required_path": "relative"}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(invalid_value),
            Err(RegistryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn enabled_config_stays_typed_and_collection_is_explicit() {
        RUNS.store(0, Ordering::SeqCst);
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.resources"],
                "config": {"required_path": "/proc"}
            }]
        }))
        .unwrap();
        let resolved = registry().resolve(selection).unwrap();
        assert_eq!(RUNS.load(Ordering::SeqCst), 0);
        let enabled = resolved.get("fixture.host").unwrap();
        assert_eq!(enabled.pack_id().as_str(), "fixture.host");
        assert_eq!(
            enabled
                .checks()
                .iter()
                .map(CheckId::as_str)
                .collect::<Vec<_>>(),
            ["host.resources"]
        );
        assert_eq!(enabled.collect::<FixturePack>().unwrap(), 1);
    }

    #[test]
    fn resolved_token_rejects_same_id_implementation_substitution() {
        SUBSTITUTION_RUNS.store(0, Ordering::SeqCst);
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": "fixture.host",
                "checks": ["host.resources"],
                "config": {"required_path": "/proc"}
            }]
        }))
        .unwrap();
        let resolved = registry().resolve(selection).unwrap();
        let enabled = resolved.get("fixture.host").unwrap();

        let parse_error = enabled
            .parse_config::<SubstitutionPack>()
            .expect_err("matching pack ID must not substitute an implementation");
        assert_eq!(parse_error.field, "implementation");
        assert!(parse_error.message.contains("FixturePack"));
        assert!(parse_error.message.contains("SubstitutionPack"));

        let collect_error = enabled
            .collect::<SubstitutionPack>()
            .expect_err("substitute collector must not run");
        assert_eq!(collect_error.field, "implementation");
        assert_eq!(SUBSTITUTION_RUNS.load(Ordering::SeqCst), 0);
    }
}
