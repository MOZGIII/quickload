//! The quickload loader implementation.

use anyhow::{anyhow, bail};
use hyper::{
    body::{Bytes, HttpBody},
    http,
};
use positioned_io_preview::RandomAccessFile;
use quickload_chunker::{CapturedChunk, TotalSize};
use quickload_disk_space_allocation as disk_space_allocation;
use std::{fs::OpenOptions, path::Path, sync::Arc};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio_util::sync::CancellationToken;

/// Issue an HTTP request with a HEAD method to the URL you need
/// to download and attempt to detect the size of the data to accomodate.
pub async fn detect_size<C>(
    client: &hyper::Client<C>,
    uri: &hyper::Uri,
) -> Result<TotalSize, anyhow::Error>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    let response = client
        .request(
            http::Request::builder()
                .method(http::Method::HEAD)
                .uri(uri.clone())
                .body(hyper::Body::empty())?,
        )
        .await?;

    let headers = response.headers();

    let size = headers
        .get(http::header::CONTENT_LENGTH)
        .ok_or_else(|| anyhow!("unable to detect size: HEAD didn't return Content-Length header"))?
        .to_str()?
        .parse()?;

    let supports_ranges = headers
        .get_all(http::header::ACCEPT_RANGES)
        .into_iter()
        .any(|val| val == "bytes");
    if !supports_ranges {
        bail!("server does not accept range requests");
    }

    Ok(size)
}

/// Initialize the file for use with the loader.
///
/// The underlying disk space will be allocated via
/// the  [`disk_space_allocation`] facilities, and the [`RandomAccessFile`]
/// will be used to interface with the filesystem.
pub fn init_file(
    into: impl AsRef<Path>,
    total_size: TotalSize,
) -> Result<RandomAccessFile, anyhow::Error> {
    let mut writer = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(into)?;
    disk_space_allocation::allocate(&mut writer, total_size)?;
    let writer = RandomAccessFile::try_new(writer)?;
    Ok(writer)
}

/// The loader.
///
/// # Cancellation
///
/// When the cancellation it triggered vie any of the cancellation tokens, the [`Loader::run`] will
/// gracefully terminate the download process as soon as possible (even if the file is not fully
/// downloaded or written).
pub struct Loader<C, W> {
    /// A hyper client we can share across multiple threads.
    pub client: Arc<hyper::Client<C>>,
    /// The the URL to download.
    pub uri: hyper::Uri,
    /// The write to write to (file for instance).
    pub writer: W,
    /// The chunk picker to define the strategy of picking the order of
    /// the chunks to download.
    pub chunker: quickload_chunker::Chunker,
    /// Cancellation token for terminating after the queued write requests are fulfilled.
    pub cancel_write_queued: CancellationToken,
    /// Cancellation token for terminating quick and dropping the pending write requests.
    pub cancel_drop_queued: CancellationToken,
}

impl<C, W> Loader<C, W>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
    W: positioned_io_preview::WriteAt + Send + 'static,
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
        } = self;

        let (write_queue_tx, write_queue_rx) = mpsc::channel(16);

        let writer_loop_handle = tokio::spawn(async move {
            let result = Self::write_loop(
                writer,
                write_queue_rx,
                cancel_write_queued,
                cancel_drop_queued,
            )
            .await;
            if let Err(error) = result {
                tracing::error!(message = "write loop error", ?error);
            }
        });

        let chunk_picker = quickload_linear_chunk_picker::Linear::new(&chunker);

        let sem = Arc::new(Semaphore::new(8));
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
            chunk_processing_set.spawn(async move {
                let result = Self::process_chunk(client, uri, chunk, write_queue_tx).await;
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
        client: Arc<hyper::Client<C>>,
        uri: hyper::Uri,
        chunk: quickload_chunker::CapturedChunk,
        write_queue: mpsc::Sender<WriteRequest>,
    ) -> Result<(), anyhow::Error> {
        let CapturedChunk {
            first_byte_offset,
            last_byte_offset,
            ..
        } = chunk;

        let range_header = format!("bytes={}-{}", first_byte_offset, last_byte_offset);

        let req = http::Request::builder()
            .uri(uri)
            .header(hyper::header::RANGE, range_header)
            .body(hyper::Body::empty())?;

        if write_queue.is_closed() {
            anyhow::bail!("write queue closed, bailing on issuing the request");
        }

        let res = tokio::select! {
            _ = write_queue.closed() => {
                anyhow::bail!("write queue closed, bailing on completing the request");
            }
            request_result = client.request(req) => request_result?,
        };
        let status = res.status();
        anyhow::ensure!(
            status == http::StatusCode::PARTIAL_CONTENT,
            "HTTP response has unexpected status {}",
            status
        );

        let content_range = res
            .headers()
            .get(http::header::CONTENT_RANGE)
            .ok_or_else(|| anyhow!("no content-range header in response"))?;
        tracing::info!(message = "loading data", content_range = %content_range.to_str()?);

        let mut body = res.into_body();

        let mut pos = first_byte_offset;
        loop {
            let data_read_result = tokio::select! {
                _ = write_queue.closed() => {
                    anyhow::bail!("write queue closed, bailing on loading more data");
                }
                maybe_data = body.data() => {
                    match maybe_data {
                        Some(data) => data,
                        None => break,
                    }
                }
            };

            let data = data_read_result?;
            let len = data.len() as TotalSize;
            let (completed_tx, completed_rx) = oneshot::channel();
            let result = write_queue
                .send(WriteRequest {
                    pos,
                    buf: data,
                    completed_tx,
                })
                .await;
            if let Err(mpsc::error::SendError(failed_request)) = result {
                // We are unable to send a request to the write queue
                // because the channel is closed.
                // The disk writer must've terminated, so we clean up.
                tracing::debug!(message = "unable to send write request", offset = %failed_request.pos);
                anyhow::bail!("unable to post a request to a write queue");
            }
            completed_rx.await?;
            pos += len;
        }
        anyhow::ensure!(
            last_byte_offset == pos - 1,
            "bytes written ({}) doen't match files expected to write ({})",
            pos - 1,
            last_byte_offset,
        );
        Ok(())
    }

    /// Run the disk write loop, serving the write queue requests and writing the data to the disk.
    async fn write_loop(
        mut writer: W,
        mut queue: mpsc::Receiver<WriteRequest>,
        // Close the queue put process all remanining write requests.
        cancel_gacefully: CancellationToken,
        // Drop the queue discarding all remaining write requests.
        cancel_flush_only: CancellationToken,
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
    pos: u64,
    /// The data to write at the given offset.
    buf: Bytes,
    /// A channel to notify the chunk loader about the completion.
    completed_tx: oneshot::Sender<()>,
}
