//! Loading progress utilities.

use quickload_chunker::{Offset, Size};

/// Progress reporter.
#[async_trait::async_trait]
pub trait Reporter {
    /// Report the progress.
    async fn report(&self, offset: Offset, data_size: Size);
}

/// A reporter that does nothing.
pub struct NoopReporter;

#[async_trait::async_trait]
impl Reporter for NoopReporter {
    async fn report(&self, _offset: Offset, _data_size: Size) {}
}

/// A reporter that emits [`tracing`] trace-level events for progress reporting.
pub struct TracingReporter {
    /// The name of the progress reporter.
    pub name: std::borrow::Cow<'static, str>,
}

impl TracingReporter {
    /// Create a new [`TracingReporter`] with a static str name.
    pub const fn from_static(name: &'static str) -> Self {
        Self {
            name: std::borrow::Cow::Borrowed(name),
        }
    }
}

#[async_trait::async_trait]
impl Reporter for TracingReporter {
    async fn report(&self, offset: Offset, data_size: Size) {
        tracing::trace!(message = "Progress", %offset, %data_size, reporter = %self.name);
    }
}
