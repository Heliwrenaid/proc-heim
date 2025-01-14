use std::fmt::Display;

use uuid::Uuid;

/// Type representing an ID of spawned process. Uses `UUID` internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId {
    inner: Uuid,
}

impl ProcessId {
    pub(crate) fn random() -> Self {
        Self {
            inner: Uuid::new_v4(),
        }
    }
}

impl Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner.to_string())
    }
}

impl From<Uuid> for ProcessId {
    fn from(inner: Uuid) -> Self {
        Self { inner }
    }
}

impl AsRef<Uuid> for ProcessId {
    fn as_ref(&self) -> &Uuid {
        &self.inner
    }
}
