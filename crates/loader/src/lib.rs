//! The quickload loader implementation.

use anyhow::{anyhow, bail};
use futures_util::poll;
use hyper::{
    body::{Bytes, HttpBody},
    http,
};
use positioned_io_preview::RandomAccessFile;
use quickload_chunker::{CapturedChunk, Config};
use quickload_disk_space_allocation as disk_space_allocation;
use std::{fs::OpenOptions, num::NonZeroU64, path::Path, sync::Arc};
use tokio::sync::{mpsc, oneshot, Semaphore};

/// The type we use for data size calculations.
pub type ByteSize = u64;

/// The type we use for chunk size.
pub type NonZeroByteSize = NonZeroU64;

/// The config type for the chunker.
#[derive(Debug)]
pub enum ChunkerConfig {}

impl Config for ChunkerConfig {
    type ChunkIndex = u64;
    type TotalSize = ByteSize;
    type ChunkSize = NonZeroByteSize;
    type Offset = ByteSize;
}

/// The loader.
pub struct Loader<C, W> {
    /// A hyper client we can share across multiple threads.
    pub client: Arc<hyper::Client<C>>,
    /// The the URL to download.
    pub uri: hyper::Uri,
    /// The write to write to (file for instance).
    pub writer: W,
    /// The chunk picker to define the strategy of picking the order of
    /// the chunks to download.
    pub chunker: quickload_chunker::Chunker<ChunkerConfig>,
}

/// Issue an HTTP request with a HEAD method to the URL you need
/// to download and attempt to detect the size of the data to accomodate.
pub async fn detect_size<C>(
    client: &hyper::Client<C>,
    uri: &hyper::Uri,
) -> Result<ByteSize, anyhow::Error>
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
    total_size: ByteSize,
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
        } = self;

        let (write_queue_tx, write_queue_rx) = mpsc::channel(16);

        let mut writer_loop_handle = tokio::spawn(Self::write_loop(writer, write_queue_rx));

        let chunk_picker = quickload_linear_chunk_picker::Linear::new(&chunker);

        let sem = Arc::new(Semaphore::new(8));
        for chunk in chunk_picker {
            // If the writer loop exited - we quit too.
            if poll!(&mut writer_loop_handle).is_ready() {
                break;
            };

            let permit = Arc::clone(&sem).acquire_owned().await;
            let write_queue_tx = write_queue_tx.clone();
            let client = Arc::clone(&client);
            let uri = uri.clone();
            tokio::spawn(async move {
                Self::process_range(client, uri, chunk, write_queue_tx)
                    .await
                    .unwrap();
                drop(permit);
            });
        }

        drop(write_queue_tx);
        writer_loop_handle.await??;

        drop(client);

        Ok(())
    }

    async fn process_range(
        client: Arc<hyper::Client<C>>,
        uri: hyper::Uri,
        chunk: quickload_chunker::CapturedChunk<ChunkerConfig>,
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

        let res = client.request(req).await?;
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
        println!("{}", content_range.to_str()?);

        let mut body = res.into_body();

        let mut pos = first_byte_offset;
        while let Some(data) = body.data().await {
            let data = data?;
            let len = data.len() as ByteSize;
            let (completed_tx, completed_rx) = oneshot::channel();
            let result = write_queue
                .send(WriteRequest {
                    pos,
                    buf: data,
                    completed_tx,
                })
                .await;
            if let Err(err) = result {
                panic!("unable to send write request: {}", err);
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

    async fn write_loop(
        mut writer: W,
        mut queue: mpsc::Receiver<WriteRequest>,
    ) -> Result<(), anyhow::Error> {
        while let Some(item) = queue.recv().await {
            let WriteRequest {
                buf,
                pos,
                completed_tx,
            } = item;
            tokio::task::block_in_place(|| writer.write_all_at(pos, buf.as_ref()))?;
            completed_tx.send(()).unwrap();
        }
        tokio::task::block_in_place(|| writer.flush())?;
        Ok(())
    }
}

struct WriteRequest {
    pos: u64,
    buf: Bytes,
    completed_tx: oneshot::Sender<()>,
}
