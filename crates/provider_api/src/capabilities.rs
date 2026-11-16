//! Capability declarations read by the runtime and UI. Nothing branches on a
//! provider name; behavior follows the declaration.

use aeris_domain::{AssetClass, ChartInterval};

/// Build targets one adapter compiles for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildTargets {
    pub native: bool,
    pub web: bool,
}

/// How a consumer signs in to the provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignInMethod {
    /// Public data, no account.
    None,
    /// A user-supplied API key.
    ApiKey,
    /// A vault-hosted login session.
    VaultLogin,
}

/// Streams the provider publishes, with their bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCapabilities {
    pub trades: bool,
    pub quotes: bool,
    /// Aggregate depth and its per-side level limit; absent means no depth.
    pub aggregate_depth_levels: Option<usize>,
    /// Order-by-order levels and the instrument cap; absent means no L3.
    pub order_level_instrument_cap: Option<usize>,
}

/// The provider's request-weight window, enforced by the shared rate budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateBudgetDeclaration {
    pub window_seconds: u32,
    /// Total weight allowed in one window.
    pub window_weight: u32,
    /// Weight background work leaves unspent for interactive requests.
    pub foreground_reserve: u32,
}

/// Everything the runtime and UI may know about a provider without naming it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub provider_id: String,
    pub display_name: String,
    pub asset_classes: Vec<AssetClass>,
    pub streams: StreamCapabilities,
    pub history_intervals: Vec<ChartInterval>,
    pub sign_in: SignInMethod,
    pub trading_venue: bool,
    pub rate_budget: RateBudgetDeclaration,
    pub targets: BuildTargets,
}
