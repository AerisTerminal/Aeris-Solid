//! Typed provider failures. The runtime reacts to the kind; the provider's
//! text is only shown to the user and is never branched on.

/// One adapter-boundary failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProviderError {
    /// Connection or I/O failure; the session may reconnect.
    #[error("transport failure: {0}")]
    Transport(String),
    /// Credentials were rejected; retrying without new credentials is futile.
    #[error("authentication failed")]
    Auth,
    /// The provider throttled the request; back off and retry later.
    #[error("rate limited")]
    RateLimited { retry_after_ms: Option<u64> },
    /// The provider refused the request itself.
    #[error("refused: {0}")]
    Refused(String),
    /// A wire payload did not match the expected schema.
    #[error("schema failure: {0}")]
    Schema(String),
    /// Provider ordering evidence skipped; a fresh snapshot is required.
    #[error("sequence gap: expected {expected}, received {actual}")]
    Gap { expected: u64, actual: u64 },
    /// The session ended cleanly and must not be reused.
    #[error("session closed")]
    Closed,
    /// The request asks for something the provider does not offer.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl ProviderError {
    /// Whether a fresh session may resolve the failure without user action.
    #[must_use]
    pub const fn reconnectable(&self) -> bool {
        matches!(
            self,
            Self::Transport(_) | Self::RateLimited { .. } | Self::Gap { .. } | Self::Closed
        )
    }
}
