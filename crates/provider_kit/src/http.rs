//! Shared blocking HTTP agent with bounded timeouts.

use std::time::Duration;
use ureq::Agent;

/// Builds an agent whose every request is bounded end to end.
#[must_use]
pub fn agent(timeout: Duration) -> Agent {
    Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}
