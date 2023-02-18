//! Resume logic.
//!
//! ## Possible implementation ideas
//!
//! - Keep the set of the chunks indexes for the chunks that have been previously successfully
//!   written to the disk; then check if the index passed is in the set.
//!
//!   - Pros:
//!     - small data, easy to store, can be optimized
//!     - no need to read the data from disk
//!     - no need to compute checksums
//!     - no need to have any sort of manifest to describe the chunks
//!     - very simple integation
//!   - Cons:
//!     - requires external bookkeeping to keep the set
//!     - requires to complete loading of each chunk to consider resume as not required
//!     - can't be used to recover a file that was corrupted (chunks are not actually checked to be
//!       valid)
//!     - does not understand the data, so can't be used to resume the download started
//!       by another app or recover a corrupted file
//!     - if the database is lost, the reconstruction is impossible without redownloading
//!       eveerything (or just assuming the file was loaded fully, which is pointless)
//!
//! - Load a list of expected checksums per chunk; read the chunk data for the given index
//!   from disk, compute a checksum and check if it matches with the expected checksum.
//!
//!   - Pros:
//!     - does not require extra bookkeeping during the download process, just the data
//!       to be stored and to be readable
//!     - very reliable, allows restoring corrupted files and resuming after other appls
//!     - simple implementation
//!   - Cons:
//!     - potentially huge waste of the disk read cycles (not an issue for SSDs), CPU cycles and
//!       energy (i.e. when working from battery) for recomputing the checksums at each resume
//!     - requires read access to the file we are writing, this might create some oddities and
//!       may complicate the implementation and affect performance; it can be done as can be seen
//!       at torrent clients, but it's not a requirement for us here.
//!     - slow and expensive
//!     - requires an externally prepared list of expected checksums, can't operate without them
//!
//! - Keep the map of a checksum by chunk index for chunks that have been previously successfully
//!   written to the disk; load a list of expected checksums per chunk, if available;
//!   if we have expected checksums, get the checksum from the map or compute on the fly by reading
//!   the data from the disk, and then check if it matches the expected one, add the checksum to
//!   the map if it does;
//!   if we don't have the expected checksums, check if the chunk index is a key in the map,
//!   ignoring the value.
//!
//!   - Pros:
//!     - works both when we have the list of expected checksums and when we don't
//!     - flexible, can switch between the two during the operation (across resumes)
//!     - less disk load, as the map caches the checksums and reduces the need to do repeating
//!       disk reads and checksum calculations
//!     - works if the bookkeeping file is lost, as long as the list of the expected checksums is
//!       available to verify the chunks checksums
//!     - can recover the file, and resume after another app - but only if the expected checksums
//!       are available
//!   - Cons:
//!     - complicated
//!     - requires bookkeeping
//!     - works best when the external checksums are available
//!     - requires access to read the data
//!
//! - Keep the map of a checksum by chunk index for chunks that are found on the disk;
//!   load a list of expected checksums per chunk; check if the gevin chunk index is the key in
//!   the list, and that the value matches the expected checksum.
//!   Map can be recomputed from the on-demand if needed, or populated as the chunks are written.
//!
//!   - Pros:
//!     - the bookkeeping allows to eliminate recomputing of the checksums
//!     - can recover the file, and resume after another app (after map recalculation)
//!     - simple
//!     - bookkeeping itself is optional, as the checksum map can technically be calculated from
//!       scratch every time, although it negates the benefits of not recomputing the checksums
//!   - Cons:
//!     - requires an externally prepared list of expected checksums, can't operate without them
//!     - requires the read access to the file (when recomputing the map)
//!

use quickload_chunker::ChunkIndex;

/// Resume state check.
///
/// If the chunk is complete the whole procedure of loading it can be skipped.
///
/// Typically, the implementation will query its internal state for a given chunk, and compare
/// it with the desired state.
pub trait Check {
    /// The check error.
    type Error;

    /// Check whether the chunk with the given index is complete.
    fn is_chunk_complete(&self, chunk_index: usize) -> Result<bool, Self::Error>;
}

/// Preserve the state for the resume checks.
pub trait Preserve {
    /// An error that can occur while preserving the resume state.
    type Error;

    /// The state (per chunk) to preserve.
    type State;

    /// Preserve the state for the given chunk.
    fn preserve(&mut self, chunk_index: ChunkIndex, state: Self::State) -> Result<(), Self::Error>;
}
