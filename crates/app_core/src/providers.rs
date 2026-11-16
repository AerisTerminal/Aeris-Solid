//! The one list of registered providers. Adding a provider means adding one
//! crate and one registration line here; no generic code changes.

use aeris_provider_api::{Catalog, History, MarketFeedProvider, ProviderCapabilities};
use aeris_provider_hyperliquid::HyperliquidProvider;
use std::sync::Arc;

/// One registered provider with the optional contracts it implements.
pub(crate) struct RegisteredProvider {
    pub capabilities: ProviderCapabilities,
    pub catalog: Option<Arc<dyn Catalog>>,
    pub history: Option<Arc<dyn History>>,
    pub feed: Option<Arc<dyn MarketFeedProvider>>,
}

/// Builds every registered provider.
pub(crate) fn registered() -> Vec<RegisteredProvider> {
    let hyperliquid = Arc::new(HyperliquidProvider::new());
    vec![RegisteredProvider {
        capabilities: HyperliquidProvider::capabilities(),
        catalog: Some(hyperliquid.clone()),
        history: Some(hyperliquid.clone()),
        feed: Some(hyperliquid),
    }]
}
