//! How the store's commit log is indexed.
//!
//! Where the pipeline stands is kept in two records, apart because they
//! change at different times: the committed path with the bounds of the log
//! it was built from ([`CommitTrackingState`](super::CommitTrackingState)),
//! and where each stage stands along it
//! ([`StageTrackingState`](super::StageTrackingState), which says what keeps
//! the two consistent).

use std::fmt::{self, Debug, Display};

/// Position of a commit segment in the store's commit log.
///
/// Every commit is stored as the path it committed under the index after the
/// one before it, so the log can be walked back a segment at a time.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CommitIndex(u64);

impl CommitIndex {
    /// The index the first commit into a fresh store goes under.
    pub fn first() -> Self {
        Self(0)
    }

    /// The index after this one.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The index before this one, if there is one.
    pub fn prev(self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

impl Display for CommitIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<u64> for CommitIndex {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<CommitIndex> for u64 {
    fn from(value: CommitIndex) -> Self {
        value.0
    }
}
