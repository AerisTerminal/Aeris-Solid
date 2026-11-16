//! History pages: bars and trades served by the provider's REST API.

use crate::error::ProviderError;
use aeris_domain::{ChartInterval, InstrumentDescriptor, MarketBar};

/// Optional trait for providers with historical bars.
pub trait History: Send + Sync {
    /// Fetches up to `limit` closed bars in `[start_unix_nanos, end_unix_nanos]`,
    /// oldest first, on the provider's history clock.
    ///
    /// # Errors
    ///
    /// Returns a typed provider failure when the page cannot be read.
    fn bars(
        &self,
        instrument: &InstrumentDescriptor,
        interval: ChartInterval,
        start_unix_nanos: i64,
        end_unix_nanos: i64,
        limit: usize,
    ) -> Result<Vec<MarketBar>, ProviderError>;
}
