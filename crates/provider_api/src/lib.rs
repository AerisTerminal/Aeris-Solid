//! The adapter contract: capability declarations, normalized events, typed
//! errors and the small optional traits a provider implements.
//!
//! An adapter only translates between a provider's wire format and the
//! normalized model. It keeps no canonical state and makes no product
//! decision; the runtimes own sessions, demand, books, candles and history.

pub mod capabilities;
pub mod catalog;
pub mod error;
pub mod feed;
pub mod history;

pub use capabilities::{
    BuildTargets, ProviderCapabilities, RateBudgetDeclaration, SignInMethod, StreamCapabilities,
};
pub use catalog::Catalog;
pub use error::ProviderError;
pub use feed::{
    FeedEvent, FeedEventSink, FeedSessionSpec, FeedSinkError, InstrumentSubscription, LiveCandle,
    MarketFeedProvider, StreamSet,
};
pub use history::History;
