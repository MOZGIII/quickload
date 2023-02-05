//! The progress reporting with the state.

use quickload_chunker::{Offset, Size, TotalSize};
use tokio::sync::{mpsc, oneshot};

/// The reporter.
#[derive(Debug)]
pub struct Reporter {
    /// The channel for reporting of the events.
    pub tx: mpsc::Sender<(Offset, Size)>,
}

#[async_trait::async_trait]
impl quickload_loader::progress::Reporter for Reporter {
    async fn report(&self, offset: Offset, data_size: Size) {
        let _ = self.tx.send((offset, data_size)).await;
    }
}

/// Manages the progress state, need to be run.
#[derive(Debug)]
pub struct StateManager {
    /// The receiver of the progress reports.
    pub progress_reports_rx: mpsc::Receiver<(Offset, Size)>,
    /// The receiver of the requests for ranges snapshots.
    pub ranges_snapshot_requests_rx: mpsc::Receiver<oneshot::Sender<Snapshot>>,
    /// The total size of the data.
    pub total_size: TotalSize,
}

/// A snapshot of the current download state.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// The total size of the data.
    pub total_size: TotalSize,
    /// A list of completed ranges of offsets.
    pub ranges: Vec<quickload_progress_state::Range>,
}

impl StateManager {
    /// Run the state manager.
    pub async fn run(self) {
        let Self {
            mut progress_reports_rx,
            mut ranges_snapshot_requests_rx,
            total_size,
        } = self;

        let mut state = quickload_progress_state::ProgressState::default();

        loop {
            tokio::select! {
                next = progress_reports_rx.recv() => {
                    match next {
                        None => {
                            // All progress reporters are gone, just stop the loop.
                            break;
                        }
                        Some(val) => Self::handle_progress_report(&mut state, val).await,
                    };
                },
                next = ranges_snapshot_requests_rx.recv() => {
                    match next {
                        None => {
                            // All range requesters are gone, just stop the loop.
                            break;
                        }
                        Some(val) => Self::handle_ranges_snapshot_request(&mut state, val, total_size).await,
                    };
                }
            };
        }
    }

    /// Handle an incoming progress report.
    async fn handle_progress_report(
        state: &mut quickload_progress_state::ProgressState,
        report: (Offset, Size),
    ) {
        let (offset, data_size) = report;
        let update_range = quickload_progress_state::Range {
            start: offset,
            end: offset + data_size,
        };
        state.merge_update(update_range);
    }

    /// Handle an incoming ranges snapshot request.
    async fn handle_ranges_snapshot_request(
        state: &mut quickload_progress_state::ProgressState,
        request: oneshot::Sender<Snapshot>,
        total_size: TotalSize,
    ) {
        if request.is_closed() {
            return;
        }
        let ranges = state.ranges().collect();
        let snapshot = Snapshot { total_size, ranges };
        let _ = request.send(snapshot);
    }
}
