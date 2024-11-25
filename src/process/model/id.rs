use std::fmt::Display;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId {
    inner: Uuid,
}

impl ProcessId {
    pub fn random() -> Self {
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
