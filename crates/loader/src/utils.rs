//! Utilities.

use anyhow::{anyhow, bail};
use hyper::http;
use positioned_io_preview::RandomAccessFile;
use quickload_chunker::{Size, TotalSize};
use quickload_disk_space_allocation as disk_space_allocation;
use std::{fs::OpenOptions, path::Path};

/// Issue an HTTP request with a HEAD method to the URL you need
/// to download and attempt to detect the size of the data to accomodate.
pub async fn detect_size<C>(
    client: &hyper::Client<C>,
    uri: &hyper::Uri,
) -> Result<Size, anyhow::Error>
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
