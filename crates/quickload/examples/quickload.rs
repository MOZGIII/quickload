//! A simple CLI app that downloads the contents at a URL and saves it into
//! a file.

use std::sync::Arc;

use clap::Parser;
use quickload_loader::NonZeroByteSize;

/// The CLI app.
#[derive(Debug, Parser)]
struct Cli {
    /// The URL to download.
    pub url: String,
    /// The file path to save the downloaded data as.
    pub file_path: String,
    /// The chunk size to use.
    #[clap(long, default_value = "4194304" /* 1024 * 1024 = 4 MB */)]
    pub chunk_size: NonZeroByteSize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Cli {
        url,
        file_path,
        chunk_size,
    } = Cli::parse();

    quickload_disk_space_allocation::prepare_privileges()?;

    let https = hyper_tls::HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let url = url.parse()?;
    let total_size = quickload_loader::detect_size(&client, &url).await?;
    let writer = quickload_loader::init_file(file_path, total_size)?;
    let chunker = quickload_chunker::Chunker {
        total_size,
        chunk_size,
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
