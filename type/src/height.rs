//! IBC client height type.
//!
//! This is a self-contained equivalent of the IBC `Height` (revision number +
//! revision height) so that this crate does not depend on any specific light
//! client framework. The ordering and display semantics match
//! `ibc.core.client.v1.Height`.

use ethereum_light_client_proto::ibc::core::client::v1::Height as ProtoHeight;

/// IBC client height, consisting of a revision number and a revision height.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Height {
    /// the revision that the client is currently on
    revision_number: u64,
    /// the height within the given revision
    revision_height: u64,
}

impl Height {
    pub fn new(revision_number: u64, revision_height: u64) -> Self {
        Self {
            revision_number,
            revision_height,
        }
    }

    pub fn revision_number(&self) -> u64 {
        self.revision_number
    }

    pub fn revision_height(&self) -> u64 {
        self.revision_height
    }
}

impl core::fmt::Display for Height {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "{}-{}", self.revision_number, self.revision_height)
    }
}

impl From<ProtoHeight> for Height {
    fn from(h: ProtoHeight) -> Self {
        Height::new(h.revision_number, h.revision_height)
    }
}

impl From<Height> for ProtoHeight {
    fn from(h: Height) -> Self {
        ProtoHeight {
            revision_number: h.revision_number,
            revision_height: h.revision_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering() {
        assert!(Height::new(0, 1) < Height::new(0, 2));
        assert!(Height::new(0, 100) < Height::new(1, 1));
        assert_eq!(Height::new(1, 1), Height::new(1, 1));
        assert!(Height::new(1, 1) < Height::new(1, 100));
    }

    #[test]
    fn test_proto_roundtrip() {
        let h = Height::new(2, 42);
        let p: ProtoHeight = h.into();
        assert_eq!(Height::from(p), h);
    }
}
