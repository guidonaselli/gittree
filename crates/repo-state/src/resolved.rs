/// Every field of `RepositoryState` and `SubmoduleState` uses this instead of
/// `Option<T>`. The whole point (spec: submodule-workspace, "no field may be
/// silently absent") is that a caller cannot accidentally treat "we don't
/// know" as "empty" or "zero" — `Resolved::Unknown` always carries the reason.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", content = "value")]
pub enum Resolved<T> {
    Known(T),
    Unknown { reason: String },
}

impl<T> Resolved<T> {
    pub fn known(value: T) -> Self {
        Resolved::Known(value)
    }

    pub fn unknown(reason: impl Into<String>) -> Self {
        Resolved::Unknown {
            reason: reason.into(),
        }
    }

    pub fn is_known(&self) -> bool {
        matches!(self, Resolved::Known(_))
    }

    pub fn as_known(&self) -> Option<&T> {
        match self {
            Resolved::Known(v) => Some(v),
            Resolved::Unknown { .. } => None,
        }
    }
}
