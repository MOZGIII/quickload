//! A simple CLI app that downloads the contents at a URL and saves it into
//! a file.

use std::sync::Arc;

use quickload_loader::ByteSize;

/// The sample chunk size.
const CHUNK_SIZE: ByteSize = 4 * 1024 * 1024; // 4 MB.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    quickload_disk_space_allocation::prepare_privileges()?;

    let mut args = std::env::args().skip(1);
    let url = args.next().ok_or("pass url as a first argument")?;
    let file_path = args.next().ok_or("pass file path as a second argument")?;

    let https = hyper_tls::HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let url = url.parse()?;
    let total_size = quickload_loader::detect_size(&client, &url).await?;
    let writer = quickload_loader::init_file(file_path, total_size)?;
    let chunker = quickload_chunker::Chunker {
        total_size,
        chunk_size: CHUNK_SIZE.try_into().unwrap(),
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let loader = quickload_loader::Loader {
        writer,
        client: Arc::new(client),
        uri: url,
        chunker,
        cancel: cancel.clone(),
    };

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        eprintln!("Got Ctrl+C, canceling");
        cancel.cancel();
    });

    loader.run().await?;

    Ok(())
}
