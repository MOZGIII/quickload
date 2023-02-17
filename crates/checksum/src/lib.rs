//! The loaded data state.

use std::fmt::Debug;

use hex::FromHex;
use quickload_chunker::ChunkIndex;
use serde::{Deserialize, Serialize};

/// The hex representation of the data.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HexData<T>(pub T)
where
    T: FromHex + AsRef<[u8]>,
    <T as FromHex>::Error: std::fmt::Display;

/// The chunk hashes.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkHashes<const LEN: usize>(pub Vec<ChunkHash<LEN>>);

/// The hash of a single chunk.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChunkHash<const LEN: usize>(#[serde(with = "hex::serde")] pub [u8; LEN]);

impl<const LEN: usize> ChunkHashes<LEN> {
    /// Get the chunk hash from the manifest.
    pub fn get(&self, index: ChunkIndex) -> Option<&[u8; LEN]> {
        let index: usize = index.try_into().unwrap();
        let val = self.0.get(index)?;
        Some(&val.0)
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;
    use serde_json::json;

    use super::*;

    fn assert_deserialize<const LEN: usize>(
        sample: serde_json::Value,
        expected: impl IntoIterator<Item = [u8; LEN]>,
    ) {
        let actual: ChunkHashes<LEN> = serde_json::from_value(sample).unwrap();
        assert_eq!(
            ChunkHashes(expected.into_iter().map(ChunkHash).collect()),
            actual
        );
    }

    #[test]
    fn deserialize() {
        assert_deserialize(json!([""]), [hex!("")]);
        assert_deserialize(json!(["", ""]), [hex!(""), hex!("")]);

        assert_deserialize(json!(["00"]), [hex!("00")]);
        assert_deserialize(json!(["0011"]), [hex!("0011")]);

        assert_deserialize(json!(["00", "11"]), [hex!("00"), hex!("11")]);
    }
}
