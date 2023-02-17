//! The chunk validation logic.

use digest::typenum;
use hyper::body::Bytes;
use quickload_chunker::ChunkIndex;

/// The type that can validate the chunk.
#[async_trait::async_trait]
pub trait ChunkValidator {
    /// The per-chunk state required by this validator.
    type State: Send + Sync;

    /// An error that can occur when initializing the chunk validator state.
    type InitError;

    /// An error that can occur when updating the chunk validator state.
    type UpdateError: ErrorEffect;

    /// An error that can occur when finalizing the chunk validation.
    type FinalizeError: ErrorEffect;

    /// Init a new validator state.
    async fn init(&self, index: ChunkIndex) -> Result<Self::State, Self::InitError>;

    /// Update the chunk validator state with new incoming data.
    async fn update(&self, state: &mut Self::State, data: Bytes) -> Result<(), Self::UpdateError>;

    /// Finalize the chunk state validation.
    async fn finalize(&self, state: Self::State) -> Result<(), Self::FinalizeError>;
}

/// The effect of the chunk validation error.
pub trait ErrorEffect {
    /// Whether the chunk is determined to be bad, or the validation procedure itself has failed.
    fn bad_chunk(&self) -> bool;
}

/// The chunk is unknown.
#[derive(Debug, thiserror::Error)]
#[error("chunk {index} not found in the chunk hashes list")]
pub struct UnknownChunkError {
    /// The index of the chunk that we failed to locate.
    pub index: ChunkIndex,
}

/// The checksum computed checksum did not match the expected checksum.
#[derive(Debug, thiserror::Error)]
#[error("checksum error: expected {expected_hash:?} for got {computed_hash:?}")]
pub struct ChecksumError<const HASH_LEN: usize> {
    /// The hash we got from the computation.
    pub computed_hash: [u8; HASH_LEN],
    /// The hash we expected to get from the computation.
    pub expected_hash: [u8; HASH_LEN],
}

impl<const HASH_LEN: usize> ErrorEffect for ChecksumError<HASH_LEN> {
    fn bad_chunk(&self) -> bool {
        true
    }
}

impl ErrorEffect for std::convert::Infallible {
    fn bad_chunk(&self) -> bool {
        // The infallible error can never occur, so the chunk is never bad.
        false
    }
}

/// The hard-coded chunk hash size.
/// This is a temporary solution for the issues with the const generics.
const CHUNK_HASH_SIZE: usize = 32;

/// The checksum chunk validator.
///
/// Computes the checksum and compares it against the known chunk hashes.
pub struct Checksum<Digest>
where
    Digest: digest::Digest<OutputSize = typenum::U<CHUNK_HASH_SIZE>>,
{
    /// The type of the digest to use.
    pub digest_type: Digest,

    /// The size of the hashsum.
    pub chunk_hashes: quickload_checksum::ChunkHashes<CHUNK_HASH_SIZE>,
}

#[async_trait::async_trait]
impl<Digest> ChunkValidator for Checksum<Digest>
where
    Digest: digest::Digest<OutputSize = typenum::U<CHUNK_HASH_SIZE>> + Send + Sync,
{
    type State = ([u8; CHUNK_HASH_SIZE], Digest);
    type InitError = UnknownChunkError;
    type UpdateError = std::convert::Infallible;
    type FinalizeError = ChecksumError<CHUNK_HASH_SIZE>;

    /// Init a new validator state.
    async fn init(&self, index: ChunkIndex) -> Result<Self::State, Self::InitError> {
        let expected_hash = self
            .chunk_hashes
            .get(index)
            .ok_or_else(|| UnknownChunkError { index })?;
        Ok((*expected_hash, Digest::new()))
    }

    /// Update the chunk validator state with new incoming data.
    async fn update(&self, state: &mut Self::State, data: Bytes) -> Result<(), Self::UpdateError> {
        tokio::task::block_in_place(|| state.1.update(&data));
        Ok(())
    }

    /// Finalize the chunk state validation.
    async fn finalize(&self, state: Self::State) -> Result<(), Self::FinalizeError> {
        let (expected_hash, digest) = state;
        let computed_hash = digest.finalize();
        if expected_hash != computed_hash.as_slice() {
            return Err(ChecksumError {
                computed_hash: computed_hash.into(),
                expected_hash,
            });
        }
        Ok(())
    }
}

/// A no-op chunk validator.
#[derive(Debug)]
pub struct Noop;

#[async_trait::async_trait]
impl ChunkValidator for Noop {
    type State = ();
    type InitError = std::convert::Infallible;
    type UpdateError = std::convert::Infallible;
    type FinalizeError = std::convert::Infallible;

    async fn init(&self, _index: ChunkIndex) -> Result<Self::State, Self::InitError> {
        Ok(())
    }

    async fn update(
        &self,
        _state: &mut Self::State,
        _data: Bytes,
    ) -> Result<(), Self::UpdateError> {
        Ok(())
    }

    async fn finalize(&self, _state: Self::State) -> Result<(), Self::FinalizeError> {
        Ok(())
    }
}
