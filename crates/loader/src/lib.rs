//! The quickload loader implementation.

use anyhow::{anyhow, bail};
use futures_util::poll;
use hyper::{
    body::{Bytes, HttpBody},
    http,
};
use positioned_io_preview::RandomAccessFile;
use quickload_disk_space_allocation as disk_space_allocation;
use std::{fs::OpenOptions, num::NonZeroU64, path::Path, sync::Arc};
use tokio::sync::{mpsc, oneshot, Semaphore};

/// The type we use for data size calculations.
pub type ByteSize = u64;

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
    pub chunk_picker: quickload_chunk_picker::linear::Linear,
}

impl<C> Loader<C, RandomAccessFile>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    /// Issue an HTTP request with a HEAD method to the URL to download and
    /// attempt to detect the size of the data to accomodate.
    pub async fn detect_size(
        into: impl AsRef<Path>,
        client: hyper::Client<C>,
        uri: hyper::Uri,
    ) -> Result<Self, anyhow::Error> {
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
            .ok_or_else(|| {
                anyhow!("unable to detect size: HEAD didn't return Content-Length header")
            })?
            .to_str()?
            .parse()?;

        let supports_ranges = headers
            .get_all(http::header::ACCEPT_RANGES)
            .into_iter()
            .any(|val| val == "bytes");
        if !supports_ranges {
            bail!("server does not accept range requests");
        }

        Self::with_size(into, client, uri, size)
    }
}

impl<C> Loader<C, RandomAccessFile> {
    /// Initialize the loader to download the provided URL storing it into
    /// a file at the `into` path, assuming the data size of `size`.
    ///
    /// The underlying disk space will be allocated via
    /// the  [`disk_space_allocation`] facilities, and the [`RandomAccessFile`]
    /// will be used to interface with the filesystem.
    pub fn with_size(
        into: impl AsRef<Path>,
        client: hyper::Client<C>,
        uri: hyper::Uri,
        size: ByteSize,
    ) -> Result<Self, anyhow::Error> {
        let client = Arc::new(client);

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(into)?;
        disk_space_allocation::allocate(&mut writer, size)?;
        let writer = RandomAccessFile::try_new(writer)?;

        let chunk_picker =
            quickload_chunk_picker::linear::Linear::new(size, NonZeroU64::new(512 * 1024).unwrap());
        Ok(Self {
            client,
            writer,
            uri,
            chunk_picker,
        })
    }
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
            chunk_picker,
            uri,
            writer,
        } = self;

        let (write_queue_tx, write_queue_rx) = mpsc::channel(16);

        let mut writer_loop_handle = tokio::spawn(Self::write_loop(writer, write_queue_rx));

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
        chunk: quickload_chunk_picker::Chunk<ByteSize>,
        write_queue: mpsc::Sender<WriteRequest>,
    ) -> Result<(), anyhow::Error> {
        let (start, end) = chunk.into_inner();

        let range_header = format!("bytes={}-{}", start, end);

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

        let mut pos = start;
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
            end == pos - 1,
            "bytes written ({}) doen't match files expected to write ({})",
            pos - 1,
            end,
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
