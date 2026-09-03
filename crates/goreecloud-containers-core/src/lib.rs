use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const MAX_CONTAINER_ID_LEN: usize = 128;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContainerId(String);

impl ContainerId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContainerIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ContainerIdError::Empty);
        }
        if value.len() > MAX_CONTAINER_ID_LEN {
            return Err(ContainerIdError::TooLong {
                actual: value.len(),
                maximum: MAX_CONTAINER_ID_LEN,
            });
        }

        for (index, character) in value.char_indices() {
            let allowed = character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.');
            if !allowed {
                return Err(ContainerIdError::InvalidCharacter { index, character });
            }
            if index == 0 && !character.is_ascii_alphanumeric() {
                return Err(ContainerIdError::InvalidStart { character });
            }
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainerIdError {
    Empty,
    TooLong { actual: usize, maximum: usize },
    InvalidStart { character: char },
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Display for ContainerIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("container identifier cannot be empty"),
            Self::TooLong { actual, maximum } => {
                write!(
                    formatter,
                    "container identifier length {actual} exceeds maximum {maximum}"
                )
            }
            Self::InvalidStart { character } => write!(
                formatter,
                "container identifier must start with an ASCII letter or digit, not '{character}'"
            ),
            Self::InvalidCharacter { index, character } => write!(
                formatter,
                "container identifier contains unsupported character '{character}' at byte index {index}"
            ),
        }
    }
}

impl Error for ContainerIdError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerState {
    Defined,
    Created,
    Running,
    Stopped,
}

impl ContainerState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defined => "defined",
            Self::Created => "created",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Defined, Self::Created)
                | (Self::Created, Self::Running)
                | (Self::Created, Self::Stopped)
                | (Self::Running, Self::Stopped)
                | (Self::Stopped, Self::Defined)
        )
    }
}

impl fmt::Display for ContainerState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerRecord {
    pub id: ContainerId,
    pub state: ContainerState,
    pub bundle_path: PathBuf,
}

impl ContainerRecord {
    #[must_use]
    pub fn new(id: ContainerId, bundle_path: PathBuf) -> Self {
        Self {
            id,
            state: ContainerState::Defined,
            bundle_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateStoreError {
    AlreadyExists(ContainerId),
    NotFound(ContainerId),
    InvalidTransition {
        id: ContainerId,
        from: ContainerState,
        to: ContainerState,
    },
}

impl fmt::Display for StateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(id) => write!(formatter, "container '{id}' already exists"),
            Self::NotFound(id) => write!(formatter, "container '{id}' was not found"),
            Self::InvalidTransition { id, from, to } => write!(
                formatter,
                "container '{id}' cannot transition from {from} to {to}"
            ),
        }
    }
}

impl Error for StateStoreError {}

pub trait StateStore {
    fn insert(&mut self, record: ContainerRecord) -> Result<(), StateStoreError>;
    fn get(&self, id: &ContainerId) -> Option<&ContainerRecord>;
    fn transition(&mut self, id: &ContainerId, next: ContainerState)
    -> Result<(), StateStoreError>;
    fn remove(&mut self, id: &ContainerId) -> Result<ContainerRecord, StateStoreError>;
    fn list(&self) -> Vec<&ContainerRecord>;
}

#[derive(Debug, Default)]
pub struct MemoryStateStore {
    records: BTreeMap<ContainerId, ContainerRecord>,
}

impl StateStore for MemoryStateStore {
    fn insert(&mut self, record: ContainerRecord) -> Result<(), StateStoreError> {
        if self.records.contains_key(&record.id) {
            return Err(StateStoreError::AlreadyExists(record.id));
        }
        self.records.insert(record.id.clone(), record);
        Ok(())
    }

    fn get(&self, id: &ContainerId) -> Option<&ContainerRecord> {
        self.records.get(id)
    }

    fn transition(
        &mut self,
        id: &ContainerId,
        next: ContainerState,
    ) -> Result<(), StateStoreError> {
        let record = self
            .records
            .get_mut(id)
            .ok_or_else(|| StateStoreError::NotFound(id.clone()))?;
        if !record.state.can_transition_to(next) {
            return Err(StateStoreError::InvalidTransition {
                id: id.clone(),
                from: record.state,
                to: next,
            });
        }
        record.state = next;
        Ok(())
    }

    fn remove(&mut self, id: &ContainerId) -> Result<ContainerRecord, StateStoreError> {
        self.records
            .remove(id)
            .ok_or_else(|| StateStoreError::NotFound(id.clone()))
    }

    fn list(&self) -> Vec<&ContainerRecord> {
        self.records.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ContainerId {
        match ContainerId::parse(value) {
            Ok(id) => id,
            Err(error) => panic!("test identifier should be valid: {error}"),
        }
    }

    #[test]
    fn validates_container_identifiers() {
        assert!(ContainerId::parse("web-01.prod").is_ok());
        assert!(matches!(
            ContainerId::parse(""),
            Err(ContainerIdError::Empty)
        ));
        assert!(matches!(
            ContainerId::parse("_hidden"),
            Err(ContainerIdError::InvalidStart { .. })
        ));
        assert!(matches!(
            ContainerId::parse("bad/id"),
            Err(ContainerIdError::InvalidCharacter { .. })
        ));
    }

    #[test]
    fn rejects_invalid_state_transition() {
        let mut store = MemoryStateStore::default();
        let container_id = id("example");
        let record = ContainerRecord::new(container_id.clone(), PathBuf::from("/tmp/example"));
        assert!(store.insert(record).is_ok());

        let error = store.transition(&container_id, ContainerState::Running);
        assert!(matches!(
            error,
            Err(StateStoreError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn permits_expected_state_sequence() {
        let mut store = MemoryStateStore::default();
        let container_id = id("example");
        let record = ContainerRecord::new(container_id.clone(), PathBuf::from("/tmp/example"));
        assert!(store.insert(record).is_ok());
        assert!(
            store
                .transition(&container_id, ContainerState::Created)
                .is_ok()
        );
        assert!(
            store
                .transition(&container_id, ContainerState::Running)
                .is_ok()
        );
        assert!(
            store
                .transition(&container_id, ContainerState::Stopped)
                .is_ok()
        );
        assert!(
            store
                .transition(&container_id, ContainerState::Defined)
                .is_ok()
        );
    }

    #[test]
    fn list_is_deterministic_by_identifier() {
        let mut store = MemoryStateStore::default();
        for name in ["zeta", "alpha", "middle"] {
            let record = ContainerRecord::new(id(name), PathBuf::from(format!("/tmp/{name}")));
            assert!(store.insert(record).is_ok());
        }

        let names: Vec<&str> = store
            .list()
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "middle", "zeta"]);
    }
}
