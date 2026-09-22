/// A value that is either known, or explicitly unknown with a reason —
/// never a silent blank.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

    pub fn into_known(self) -> Option<T> {
        match self {
            Resolved::Known(v) => Some(v),
            Resolved::Unknown { .. } => None,
        }
    }
}
