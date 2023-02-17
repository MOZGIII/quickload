//! The quickload loader implementation.

pub mod chunk_validator;
mod http_partial_loader;
pub mod progress;
mod utils;

use futures_util::{future::Either, TryFutureExt};
use hyper::body::{Bytes, HttpBody};
use quickload_chunker::{CapturedChunk, Offset, Size};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::chunk_validator::ErrorEffect;

pub use self::utils::*;

/// The loader.
///
/// # Cancellation
///
/// When the cancellation it triggered vie any of the cancellation tokens, the [`Loader::run`] will
/// gracefully terminate the download process as soon as possible (even if the file is not fully
/// downloaded or written).
pub struct Loader<Connect, Writer, NetProgressReporter, DiskProgressReporter, ChunkValidator> {
    /// A hyper client we can share across multiple threads.
    pub client: Arc<hyper::Client<Connect>>,
    /// The the URL to download.
    pub uri: hyper::Uri,
    /// The write to write to (file for instance).
    pub writer: Writer,
    /// The chunk picker to define the strategy of picking the order of
    /// the chunks to download.
    pub chunker: quickload_chunker::Chunker,
    /// Cancellation token for terminating after the queued write requests are fulfilled.
    pub cancel_write_queued: CancellationToken,
    /// Cancellation token for terminating quick and dropping the pending write requests.
    pub cancel_drop_queued: CancellationToken,
    /// The progress reporter for the networking side.
    pub net_progress_reporter: Arc<NetProgressReporter>,
    /// The progress reporter for the disk side.
    pub disk_progress_reporter: DiskProgressReporter,
    /// The chunk validator to use.
    pub chunk_validator: Arc<ChunkValidator>,
    /// Max amount of retries to download the chunk.
    pub max_chunk_load_retries: usize,
    /// Networking concurrency.
    pub net_concurrency: usize,
}

impl<Connect, Writer, NetProgressReporter, DiskProgressReporter, ChunkValidator>
    Loader<Connect, Writer, NetProgressReporter, DiskProgressReporter, ChunkValidator>
where
    Connect: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
    Writer: positioned_io_preview::WriteAt + Send + 'static,
    NetProgressReporter: progress::Reporter + Send + Sync + 'static,
    DiskProgressReporter: progress::Reporter + Send + Sync + 'static,
    ChunkValidator: chunk_validator::ChunkValidator + Send + Sync + 'static,
    <ChunkValidator as chunk_validator::ChunkValidator>::InitError:
        std::error::Error + Send + Sync + 'static,
    <ChunkValidator as chunk_validator::ChunkValidator>::UpdateError:
        std::error::Error + Send + Sync + 'static,
    <ChunkValidator as chunk_validator::ChunkValidator>::FinalizeError:
        std::error::Error + Send + Sync + 'static,
{
    /// Run the download operation represented by this loader.
    pub async fn run(self) -> Result<(), anyhow::Error> {
        let Self {
            client,
            chunker,
            uri,
            writer,
            cancel_write_queued,
            cancel_drop_queued,
            net_progress_reporter,
            disk_progress_reporter,
            chunk_validator,
            max_chunk_load_retries,
            net_concurrency,
        } = self;

        let (write_queue_tx, write_queue_rx) = mpsc::channel(net_concurrency * 2);

        let writer_loop_handle = tokio::spawn(async move {
            let result = Self::write_loop(
                writer,
                write_queue_rx,
                cancel_write_queued,
                cancel_drop_queued,
                disk_progress_reporter,
            )
            .await;
            if let Err(error) = result {
                tracing::error!(message = "write loop error", ?error);
            }
        });

        let chunk_picker = quickload_linear_chunk_picker::Linear::new(&chunker);

        let sem = Arc::new(Semaphore::new(net_concurrency));
        let mut chunk_processing_set = tokio::task::JoinSet::new();
        for chunk in chunk_picker {
            // If the writer queue is closed we can safely quit - there is no way we can submit
            // more data to write, so why bother loading it.
            if write_queue_tx.is_closed() {
                break;
            };

            let permit = Arc::clone(&sem).acquire_owned().await;
            let write_queue_tx = write_queue_tx.clone();
            let client = Arc::clone(&client);
            let uri = uri.clone();
            let net_progress_reporter = Arc::clone(&net_progress_reporter);
            let chunk_validator = Arc::clone(&chunk_validator);
            chunk_processing_set.spawn(async move {
                let mut retries = max_chunk_load_retries;

                let result = loop {
                    let result = Self::process_chunk(
                        Arc::clone(&client),
                        uri.clone(),
                        chunk,
                        write_queue_tx.clone(),
                        Arc::clone(&net_progress_reporter),
                        Arc::clone(&chunk_validator),
                    )
                    .await;
                    let result = match result {
                        Ok(val) => Ok(val),
                        Err(ErrorBehavior::Retriable(err)) => {
                            if retries > 0 {
                                retries -= 1;
                                tracing::warn!(
                                    message = "got a retriable error while loading the chunk, retrying",
                                    retires_left = %retries,
                                    ?chunk,
                                    error = %err,
                                );
                                continue;
                            }
                            Err(err)
                        }
                        Err(ErrorBehavior::NonRetriable(err)) => Err(err),
                    };
                    break result;
                };
                drop(permit);
                if let Err(error) = result {
                    tracing::error!(message = "chunk processing error", ?error, ?chunk);
                }
            });
        }

        // The chunk loading routine has finished, so we can stop holding for the write queue tx as
        // we won't be posting any more work to it.
        drop(write_queue_tx);

        // Wait for the write loop to finish and propagate the panic (if any).
        writer_loop_handle.await.unwrap();

        // Wait for the gaceful termination of all the chunk processing routines.
        while let Some(chunk_processing_result) = chunk_processing_set.join_next().await {
            // Propagate the panics, if any.
            chunk_processing_result.unwrap();
        }

        drop(client);

        Ok(())
    }

    /// Process the given chunk by issuing a data download request, and then submitting the data
    /// received from the request to the disk writing queue.
    async fn process_chunk(
        client: Arc<hyper::Client<Connect>>,
        uri: hyper::Uri,
        chunk: quickload_chunker::CapturedChunk,
        write_queue: mpsc::Sender<WriteRequest>,
        progress_reporter: Arc<NetProgressReporter>,
        chunk_validator: Arc<ChunkValidator>,
    ) -> Result<(), ErrorBehavior<ProcessChunkError<ChunkValidator>>> {
        if write_queue.is_closed() {
            return Err(ErrorBehavior::NonRetriable(
                ProcessChunkError::WriteQueueClosed("precheck"),
            ));
        }

        let CapturedChunk {
            first_byte_offset,
            last_byte_offset,
            index,
            ..
        } = chunk;

        let progress_reporter = progress_reporter.as_ref();

        let loader = http_partial_loader::Loader {
            client,
            byte_range: (first_byte_offset, last_byte_offset),
            uri,
        };

        let validation_state_handle: abort_on_drop::ChildTask<_> = {
            let chunk_validator = Arc::clone(&chunk_validator);
            tokio::spawn(async move { chunk_validator.init(index).await }).into()
        };
        let handshake_handle: abort_on_drop::ChildTask<_> = tokio::spawn(loader.handshake()).into();

        let init_fut = async {
            let validation_state = validation_state_handle.await.unwrap().map_err(|err| {
                ErrorBehavior::NonRetriable(ProcessChunkError::ValidationStateInit(err))
            })?;
            let body = handshake_handle.await.unwrap().map_err(|err| {
                let is_retriable =
                    matches!(&err, http_partial_loader::HandshakeError::Request(err) if err.is_incomplete_message() || err.is_timeout());
                if is_retriable {
                    ErrorBehavior::Retriable(ProcessChunkError::LoaderHandshake(err))
                } else {
                    ErrorBehavior::NonRetriable(ProcessChunkError::LoaderHandshake(err))
                }
            })?;
            Ok((body, validation_state))
        };

        let (mut body, mut validation_state) = tokio::select! {
            _ = write_queue.closed() => return Err(ErrorBehavior::NonRetriable(ProcessChunkError::WriteQueueClosed("init"))),
            init_result = init_fut => init_result?,
        };

        let mut pos = first_byte_offset;
        loop {
            let data = tokio::select! {
                _ = write_queue.closed() => return Err(ErrorBehavior::NonRetriable(ProcessChunkError::WriteQueueClosed("data"))),
                maybe_data = body.data() => {
                    match maybe_data {
                        Some(Ok(data)) => data,
                        Some(Err(error)) => return Err(ErrorBehavior::Retriable(ProcessChunkError::LoaderData(error))),
                        None => break,
                    }
                }
            };

            let len = data.len() as Size;
            let (completed_tx, completed_rx) = oneshot::channel();

            let validator_update_result = chunk_validator
                .update(&mut validation_state, data.clone())
                .map_err(Either::Left);
            let write_queue_send_result = write_queue
                .send(WriteRequest {
                    pos,
                    buf: data,
                    completed_tx,
                })
                .map_err(Either::Right);

            let result = tokio::try_join![validator_update_result, write_queue_send_result];
            match result {
                Err(Either::Right(mpsc::error::SendError(failed_request))) => {
                    // We are unable to send a request to the write queue
                    // because the channel is closed.
                    // The disk writer must've terminated, so we clean up.
                    tracing::debug!(message = "unable to send write request", offset = %failed_request.pos);
                    return Err(ErrorBehavior::NonRetriable(
                        ProcessChunkError::WriteQueueClosed("posting to write queue"),
                    ));
                }
                Err(Either::Left(update_error)) => {
                    if update_error.bad_chunk() {
                        tracing::debug!(message = "chunk validator reported bad chunk, restarting chunk loading", error = %update_error);
                        return Err(ErrorBehavior::Retriable(
                            ProcessChunkError::ValidationStateUpdate(update_error),
                        ));
                    }
                    return Err(ErrorBehavior::NonRetriable(
                        ProcessChunkError::ValidationStateUpdate(update_error),
                    ));
                }
                Ok(_) => {}
            }
            progress_reporter.report(pos, len).await;
            completed_rx.await.map_err(|error| {
                ErrorBehavior::NonRetriable(ProcessChunkError::WriteCompletionChannelClosed(error))
            })?;
            pos += len;
        }
        let last_written_offset = pos - 1;
        if last_byte_offset != last_written_offset {
            return Err(ErrorBehavior::Retriable(ProcessChunkError::SizeMismatch {
                last_written_offset,
                expected_last_written_offset: last_byte_offset,
            }));
        }

        // Before completing the processing, verify the chunk.
        chunk_validator
            .finalize(validation_state)
            .await
            .map_err(|err| {
                ErrorBehavior::Retriable(ProcessChunkError::ValidationStateFinalize(err))
            })?;

        Ok(())
    }

    /// Run the disk write loop, serving the write queue requests and writing the data to the disk.
    async fn write_loop(
        mut writer: Writer,
        mut queue: mpsc::Receiver<WriteRequest>,
        // Close the queue put process all remanining write requests.
        cancel_gacefully: CancellationToken,
        // Drop the queue discarding all remaining write requests.
        cancel_flush_only: CancellationToken,
        // The progress reporter,
        progress_reporter: DiskProgressReporter,
    ) -> Result<(), anyhow::Error> {
        let mut is_cancelling_gracefully = false;
        loop {
            let maybe_item = tokio::select! {
                _ = cancel_flush_only.cancelled() => {
                    // Drop the queue, break the loop and go into flush.
                    drop(queue);
                    break;
                }
                _ = cancel_gacefully.cancelled(), if !is_cancelling_gracefully => {
                    // Mark that we don't need to poll for graceful cancellation anymore as
                    // it completes immediately when already cancelled, close the queue and
                    // continue with the loop processing the remaining write requests.
                    is_cancelling_gracefully = true;
                    queue.close();
                    continue;
                }
                maybe_item = queue.recv() => maybe_item
            };
            let Some(item) = maybe_item else {
                break;
            };
            let WriteRequest {
                buf,
                pos,
                completed_tx,
            } = item;
            tokio::task::block_in_place(|| writer.write_all_at(pos, buf.as_ref()))?;

            let completion_notification_result = completed_tx.send(());
            if let Err(()) = completion_notification_result {
                // We were unable to issue a write completion notification to the write requester
                // because the channel has been closed (or already used).
                // This is no big deal, we'll just log it and continue serving the queue.
                tracing::debug!(message = "unable to send a write completion notification");
            }

            progress_reporter.report(pos, buf.len() as Size).await;
        }
        tokio::task::block_in_place(|| writer.flush())?;
        Ok(())
    }
}

/// The request for writing the data into a file.
/// This is a private implementation detail of the interation between the chunk processing logic
/// and the write loop.
struct WriteRequest {
    /// The offset in the file.
    pos: Offset,
    /// The data to write at the given offset.
    buf: Bytes,
    /// A channel to notify the chunk loader about the completion.
    completed_tx: oneshot::Sender<()>,
}

/// Whether the error is retriable or not.
#[derive(Debug, thiserror::Error)]
enum ErrorBehavior<T> {
    /// The processing should be retried.
    #[error(transparent)]
    Retriable(T),
    /// The processing should not be retried.
    #[error(transparent)]
    NonRetriable(T),
}

/// The chunk processing error.
#[derive(derivative::Derivative, thiserror::Error)]
#[derivative(Debug)]
enum ProcessChunkError<ChunkValidator>
where
    ChunkValidator: chunk_validator::ChunkValidator,
    <ChunkValidator as chunk_validator::ChunkValidator>::InitError:
        std::error::Error + Send + Sync + 'static,
    <ChunkValidator as chunk_validator::ChunkValidator>::UpdateError:
        std::error::Error + Send + Sync + 'static,
    <ChunkValidator as chunk_validator::ChunkValidator>::FinalizeError:
        std::error::Error + Send + Sync + 'static,
{
    /// The write queue was closed.
    #[error("write queue closed ({0})")]
    WriteQueueClosed(&'static str),
    /// Error while conducting the initial handshake.
    #[error(transparent)]
    LoaderHandshake(http_partial_loader::HandshakeError),
    /// Error while initializing the chunk validation state.
    #[error(transparent)]
    ValidationStateInit(ChunkValidator::InitError),
    /// Error while loading the data.
    #[error("loading error: {0}")]
    LoaderData(hyper::Error),
    /// Error while updating the chunk validation state.
    #[error(transparent)]
    ValidationStateUpdate(ChunkValidator::UpdateError),
    /// Error while finalizing the chunk validation state.
    #[error(transparent)]
    ValidationStateFinalize(ChunkValidator::FinalizeError),
    /// The downloaded data does not match the chunk size.
    #[error("last written offset ({last_written_offset}) doesn't match the expected offset ({expected_last_written_offset})")]
    SizeMismatch {
        /// The last data offset that was affected by the write requests that we sent.
        last_written_offset: Offset,
        /// The expected last data offset to be affected by the loading of this chunk.
        expected_last_written_offset: Offset,
    },
    /// The write completion channel was closed before we got the confirmation.
    #[error("write completion channel closed")]
    WriteCompletionChannelClosed(oneshot::error::RecvError),
}
