use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadClipboardSemantic,
    ReadSelectionSemantic,
    ReadFocusSemantic,
    ReadClipboardContent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    ForegroundApp,
    Session,
    Persistent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionRequest {
    pub capability: Capability,
    pub scope: Scope,
    pub reason: String,
    pub ttl: Option<Duration>,
}

impl PermissionRequest {
    pub fn new(capability: Capability, scope: Scope, reason: impl Into<String>) -> Self {
        Self {
            capability,
            scope,
            reason: reason.into(),
            ttl: None,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    pub capability: Capability,
    pub scope: Scope,
    pub reason: String,
    #[serde(with = "system_time_serde")]
    pub granted_at: SystemTime,
    #[serde(with = "option_system_time_serde")]
    pub expires_at: Option<SystemTime>,
}

impl Grant {
    pub fn is_active_at(&self, now: SystemTime) -> bool {
        self.expires_at
            .map(|expires_at| expires_at > now)
            .unwrap_or(true)
    }
}

pub struct PermissionStore {
    grants: HashMap<Capability, Grant>,
    persistence: Option<JsonFilePersistence>,
}

impl Default for PermissionStore {
    fn default() -> Self {
        Self {
            grants: HashMap::new(),
            persistence: None,
        }
    }
}

impl PermissionStore {
    pub(crate) fn with_defaults() -> Self {
        let mut store = Self::default();
        for capability in [
            Capability::ReadClipboardSemantic,
            Capability::ReadSelectionSemantic,
            Capability::ReadFocusSemantic,
        ] {
            store.grant_internal(PermissionRequest::new(
                capability,
                Scope::Session,
                "Structural signals are safe by default",
            ));
        }
        store
    }

    pub fn with_persistence(path: PathBuf) -> Result<Self, Error> {
        let persistence = JsonFilePersistence::new(path);
        let mut store = Self {
            grants: HashMap::new(),
            persistence: Some(persistence),
        };

        // Load existing persistent grants
        store.load_persistent_grants()?;

        // Add default semantic grants
        for capability in [
            Capability::ReadClipboardSemantic,
            Capability::ReadSelectionSemantic,
            Capability::ReadFocusSemantic,
        ] {
            if !store.grants.contains_key(&capability) {
                store.grant_internal(PermissionRequest::new(
                    capability,
                    Scope::Session,
                    "Structural signals are safe by default",
                ));
            }
        }

        Ok(store)
    }

    pub fn default_persistence_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lcsa")
            .join("permissions.json")
    }

    fn load_persistent_grants(&mut self) -> Result<(), Error> {
        if let Some(ref persistence) = self.persistence {
            match persistence.load() {
                Ok(grants) => {
                    let now = SystemTime::now();
                    for grant in grants {
                        // Only load grants that are still active and persistent
                        if grant.scope == Scope::Persistent && grant.is_active_at(now) {
                            self.grants.insert(grant.capability, grant);
                        }
                    }
                }
                Err(Error::PersistenceNotFound) => {
                    // File doesn't exist yet, that's fine
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn save_persistent_grants(&self) -> Result<(), Error> {
        if let Some(ref persistence) = self.persistence {
            let persistent_grants: Vec<&Grant> = self
                .grants
                .values()
                .filter(|g| g.scope == Scope::Persistent)
                .collect();
            persistence.save(&persistent_grants)?;
        }
        Ok(())
    }

    pub(crate) fn grant(&mut self, request: PermissionRequest) -> Grant {
        let grant = self.grant_internal(request);

        // Persist if this is a persistent grant
        if grant.scope == Scope::Persistent {
            let _ = self.save_persistent_grants();
        }

        grant
    }

    fn grant_internal(&mut self, request: PermissionRequest) -> Grant {
        let granted_at = SystemTime::now();
        let expires_at = request.ttl.and_then(|ttl| granted_at.checked_add(ttl));

        let grant = Grant {
            capability: request.capability,
            scope: request.scope,
            reason: request.reason,
            granted_at,
            expires_at,
        };

        self.grants.insert(grant.capability, grant.clone());
        grant
    }

    pub(crate) fn is_granted(&self, capability: Capability) -> bool {
        self.grants
            .get(&capability)
            .map(|grant| grant.is_active_at(SystemTime::now()))
            .unwrap_or(false)
    }

    pub(crate) fn revoke(&mut self, capability: Capability) -> bool {
        let existed = self.grants.remove(&capability).is_some();
        if existed {
            let _ = self.save_persistent_grants();
        }
        existed
    }
}

struct JsonFilePersistence {
    path: PathBuf,
}

impl JsonFilePersistence {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> Result<Vec<Grant>, Error> {
        if !self.path.exists() {
            return Err(Error::PersistenceNotFound);
        }

        let file = File::open(&self.path).map_err(|e| {
            Error::PersistenceError(format!("failed to open {}: {}", self.path.display(), e))
        })?;
        let reader = BufReader::new(file);
        let grants: Vec<Grant> = serde_json::from_reader(reader).map_err(|e| {
            Error::PersistenceError(format!("failed to parse {}: {}", self.path.display(), e))
        })?;
        Ok(grants)
    }

    fn save(&self, grants: &[&Grant]) -> Result<(), Error> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::PersistenceError(format!("failed to create dir {}: {}", parent.display(), e))
            })?;
        }

        let file = File::create(&self.path).map_err(|e| {
            Error::PersistenceError(format!("failed to create {}: {}", self.path.display(), e))
        })?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &grants).map_err(|e| {
            Error::PersistenceError(format!("failed to write {}: {}", self.path.display(), e))
        })?;
        Ok(())
    }
}

mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

mod option_system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &Option<SystemTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match time {
            Some(t) => {
                let duration = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
                Some(duration.as_secs()).serialize(serializer)
            }
            None => None::<u64>.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SystemTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<u64> = Option::deserialize(deserializer)?;
        Ok(secs.map(|s| UNIX_EPOCH + Duration::from_secs(s)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    #[test]
    fn default_store_grants_semantic_clipboard_access() {
        let store = PermissionStore::with_defaults();
        assert!(store.is_granted(Capability::ReadClipboardSemantic));
        assert!(store.is_granted(Capability::ReadSelectionSemantic));
        assert!(store.is_granted(Capability::ReadFocusSemantic));
        assert!(!store.is_granted(Capability::ReadClipboardContent));
    }

    #[test]
    fn ttl_grant_expires() {
        let granted_at = SystemTime::UNIX_EPOCH;
        let grant = Grant {
            capability: Capability::ReadClipboardContent,
            scope: Scope::Session,
            reason: "test".to_string(),
            granted_at,
            expires_at: Some(granted_at + Duration::from_secs(5)),
        };

        assert!(grant.is_active_at(granted_at + Duration::from_secs(4)));
        assert!(!grant.is_active_at(granted_at + Duration::from_secs(5)));
    }

    #[test]
    fn grant_serializes_to_json() {
        let grant = Grant {
            capability: Capability::ReadClipboardContent,
            scope: Scope::Persistent,
            reason: "test persistence".to_string(),
            granted_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1000),
            expires_at: None,
        };

        let json = serde_json::to_string(&grant).expect("serialize");
        assert!(json.contains("read_clipboard_content"));
        assert!(json.contains("persistent"));

        let parsed: Grant = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.capability, grant.capability);
        assert_eq!(parsed.scope, grant.scope);
    }

    #[test]
    fn persistence_round_trip() {
        let temp_dir = std::env::temp_dir().join("lcsa_test");
        let path = temp_dir.join("test_permissions.json");

        // Clean up from previous runs
        let _ = std::fs::remove_file(&path);

        // Create store with persistence and add a persistent grant
        let mut store = PermissionStore::with_persistence(path.clone()).expect("create store");
        store.grant(PermissionRequest::new(
            Capability::ReadClipboardContent,
            Scope::Persistent,
            "test persistence",
        ));

        // Create new store and verify grant was loaded
        let store2 = PermissionStore::with_persistence(path.clone()).expect("reload store");
        assert!(store2.is_granted(Capability::ReadClipboardContent));

        // Clean up
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
