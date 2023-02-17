//! A simple CLI app that downloads the contents at a URL and saves it into
//! a file.

use std::sync::Arc;

use clap::Parser;
use quickload_chunker::{ChunkSize, TotalSize};

/// The CLI app.
#[derive(Debug, Parser)]
struct Cli {
    /// The URL to download.
    pub url: String,
    /// The file path to save the downloaded data as.
    pub file_path: String,
    /// The chunk size to use.
    #[clap(long, default_value = "4194304" /* 1024 * 1024 = 4 MB */)]
    pub chunk_size: ChunkSize,
    /// Max retries to load a chunk (per each chunk).
    #[clap(long, default_value_t = 3)]
    pub max_chunk_load_retries: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Cli {
        url,
        file_path,
        chunk_size,
        max_chunk_load_retries,
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
    let cancel_write_queued = tokio_util::sync::CancellationToken::new();
    let cancel_drop_queued = tokio_util::sync::CancellationToken::new();
    let loader = quickload_loader::Loader {
        writer,
        client: Arc::new(client),
        uri: url,
        chunker,
        cancel_write_queued: cancel_write_queued.clone(),
        cancel_drop_queued: cancel_drop_queued.clone(),
        net_progress_reporter: Arc::new(reporter("net", total_size)),
        disk_progress_reporter: reporter("disk", total_size),
        chunk_validator: Arc::new(quickload_loader::chunk_validator::Noop),
        max_chunk_load_retries,
    };

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        eprintln!("Got Ctrl+C, canceling");
        cancel_write_queued.cancel();
        tokio::signal::ctrl_c().await.unwrap();
        eprintln!("Got Ctrl+C again, canceling with dropping the write queue");
        cancel_drop_queued.cancel();
    });

    loader.run().await?;

    Ok(())
}

/// Create a reporter that will report the progress to the command like periodically.
pub fn reporter(
    name: &'static str,
    total_size: TotalSize,
) -> impl quickload_loader::progress::Reporter {
    let (progress_reports_tx, progress_reports_rx) = tokio::sync::mpsc::channel(1);
    let (ranges_snapshot_requests_tx, ranges_snapshot_requests_rx) = tokio::sync::mpsc::channel(1);

    let manager = quickload_progress_state_reporter::StateManager {
        progress_reports_rx,
        ranges_snapshot_requests_rx,
        total_size,
    };
    tokio::spawn(manager.run());

    tokio::spawn(async move {
        loop {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if ranges_snapshot_requests_tx.send(tx).await.is_err() {
                break;
            }
            let snapshot = match rx.await {
                Ok(val) => val,
                Err(_) => break,
            };

            tracing::info!(message = "Progress", reporter = %name, total_size = %snapshot.total_size, ranges = %format_ranges(&snapshot));
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    quickload_progress_state_reporter::Reporter {
        tx: progress_reports_tx,
    }
}

/// Format the ranges into a more compact representation.
fn format_ranges(snapshot: &quickload_progress_state_reporter::Snapshot) -> String {
    let mut s: String = snapshot
        .ranges
        .iter()
        .map(|range| format!("{}-{},", range.start, range.end))
        .collect();
    s.truncate(s.trim_end_matches(',').len());
    s
}
