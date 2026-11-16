//! Instrument search and resolution.

use crate::error::ProviderError;
use aeris_domain::InstrumentDescriptor;

/// Optional trait for providers with an instrument catalog.
pub trait Catalog: Send + Sync {
    /// Searches instruments by free text, returning at most `limit` matches.
    /// An empty query lists the provider's leading instruments.
    ///
    /// # Errors
    ///
    /// Returns a typed provider failure when the catalog cannot be read.
    fn search(&self, query: &str, limit: usize)
    -> Result<Vec<InstrumentDescriptor>, ProviderError>;
}
