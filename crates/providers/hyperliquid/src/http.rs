//! Bounded HTTP work over the public info endpoint.
//!
//! Every call is an unauthenticated `POST /info` request with explicit byte
//! and time bounds, spending from the provider's shared request weight
//! budget. Responses stay raw bytes so fractional decimals never pass
//! through `f64` before the fixed-point decoders see them.

use crate::candles::{
    HyperliquidCandlePage, decode_candle_page, hyperliquid_interval_for_chart,
    period_for_hyperliquid_interval,
};
use aeris_domain::{ChartInterval, InstrumentDescriptor};
use aeris_provider_api::ProviderError;
use aeris_provider_kit::rate_budget::BudgetError;
use aeris_provider_kit::{RateBudget, RequestPriority};
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Public info endpoint used for metadata and candles.
pub const INFO_URL: &str = "https://api.hyperliquid.xyz/info";

/// Maximum response bytes accepted before failing closed.
const MAXIMUM_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum request body bytes sent.
const MAXIMUM_REQUEST_BYTES: usize = 64 * 1024;
/// Weight of every info request this adapter sends.
const INFO_REQUEST_WEIGHT: u32 = 20;
/// `candleSnapshot` adds one weight per this many returned candles.
const CANDLES_PER_WEIGHT: usize = 60;
/// Longest a request waits for budget before failing as rate limited.
const MAXIMUM_BUDGET_WAIT: Duration = Duration::from_secs(10);
/// End-to-end bound for one info request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Reusable info client sharing one request weight budget.
pub struct InfoClient {
    agent: ureq::Agent,
    budget: Arc<RateBudget>,
}

impl InfoClient {
    #[must_use]
    pub fn new(budget: Arc<RateBudget>) -> Self {
        Self {
            agent: aeris_provider_kit::http::agent(REQUEST_TIMEOUT),
            budget,
        }
    }

    /// Executes one `POST /info` request and returns the raw response bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for an oversized request, an exhausted budget,
    /// transport failures, or over-bound or unreadable responses.
    pub fn post(
        &self,
        body: &serde_json::Value,
        priority: RequestPriority,
        stop: Option<&AtomicBool>,
    ) -> Result<Vec<u8>, ProviderError> {
        let raw = body.to_string();
        if raw.len() > MAXIMUM_REQUEST_BYTES {
            return Err(ProviderError::Refused(
                "hyperliquid info request is too large".to_string(),
            ));
        }
        self.budget
            .spend(INFO_REQUEST_WEIGHT, priority, stop, MAXIMUM_BUDGET_WAIT)
            .map_err(|error| match error {
                BudgetError::Cancelled => ProviderError::Closed,
                BudgetError::Exhausted => ProviderError::RateLimited {
                    retry_after_ms: None,
                },
            })?;
        let mut response = self
            .agent
            .post(INFO_URL)
            .header("Content-Type", "application/json")
            .send(raw)
            .map_err(|error| {
                if matches!(error, ureq::Error::StatusCode(429)) {
                    self.budget.provider_limited();
                }
                info_request_error(error)
            })?;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAXIMUM_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                ProviderError::Transport("hyperliquid info response is unreadable".to_string())
            })?;
        if bytes.len() > MAXIMUM_RESPONSE_BYTES {
            return Err(ProviderError::Schema(
                "hyperliquid info response exceeds the bound".to_string(),
            ));
        }
        Ok(bytes)
    }

    /// Executes one info request and parses the response as routing JSON.
    ///
    /// # Errors
    ///
    /// Returns the request failure or a schema failure for malformed JSON.
    pub fn post_json(
        &self,
        body: &serde_json::Value,
        priority: RequestPriority,
        stop: Option<&AtomicBool>,
    ) -> Result<serde_json::Value, ProviderError> {
        let bytes = self.post(body, priority, stop)?;
        serde_json::from_slice(&bytes).map_err(|_| {
            ProviderError::Schema("hyperliquid info response is malformed".to_string())
        })
    }

    /// Fetches one bounded candle window for an instrument.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for invalid windows, unsupported intervals,
    /// transport failures, or malformed candle pages.
    pub fn candle_snapshot(
        &self,
        instrument: &InstrumentDescriptor,
        interval: ChartInterval,
        start_millis: i64,
        end_millis: i64,
        now_millis: i64,
        stop: &AtomicBool,
    ) -> Result<HyperliquidCandlePage, ProviderError> {
        let coin = &instrument.symbol;
        if coin.trim().is_empty()
            || coin.len() > 96
            || start_millis < 0
            || end_millis <= start_millis
        {
            return Err(ProviderError::Refused(
                "hyperliquid candle request is invalid".to_string(),
            ));
        }
        let interval_text =
            hyperliquid_interval_for_chart(interval).map_err(ProviderError::Unsupported)?;
        let period =
            period_for_hyperliquid_interval(interval_text).map_err(ProviderError::Unsupported)?;
        let bytes = self.post(
            &serde_json::json!({
                "type": "candleSnapshot",
                "req": {
                    "coin": coin,
                    "interval": interval_text,
                    "startTime": start_millis,
                    "endTime": end_millis,
                },
            }),
            RequestPriority::Interactive,
            Some(stop),
        )?;
        let text = String::from_utf8(bytes).map_err(|_| {
            ProviderError::Schema("hyperliquid info response is malformed".to_string())
        })?;
        let payload = serde_json::value::RawValue::from_string(text).map_err(|_| {
            ProviderError::Schema("hyperliquid info response is malformed".to_string())
        })?;
        let page = decode_candle_page(
            &payload,
            period,
            u32::from(instrument.price_scale),
            u32::from(instrument.quantity_scale),
            now_millis,
        )
        .map_err(ProviderError::Schema)?;
        let returned = page.bars.len() + usize::from(page.forming.is_some());
        self.budget
            .charge(u32::try_from(returned.div_ceil(CANDLES_PER_WEIGHT)).unwrap_or(u32::MAX));
        Ok(page)
    }
}

/// Preserves actionable transport/status classes without forwarding response
/// bodies, URLs, proxy configuration, or arbitrary provider text.
fn info_request_error(error: ureq::Error) -> ProviderError {
    match error {
        ureq::Error::StatusCode(429) => ProviderError::RateLimited {
            retry_after_ms: None,
        },
        ureq::Error::StatusCode(status) => {
            ProviderError::Refused(format!("hyperliquid info request failed: HTTP {status}"))
        }
        ureq::Error::Timeout(_) => {
            ProviderError::Transport("hyperliquid info request timed out".to_string())
        }
        ureq::Error::HostNotFound => {
            ProviderError::Transport("hyperliquid info host could not be resolved".to_string())
        }
        ureq::Error::Io(error) => ProviderError::Transport(format!(
            "hyperliquid info transport failed: {:?}",
            error.kind()
        )),
        ureq::Error::ConnectionFailed => {
            ProviderError::Transport("hyperliquid info connection failed".to_string())
        }
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => {
            ProviderError::Transport("hyperliquid info TLS failed".to_string())
        }
        _ => ProviderError::Transport("hyperliquid info request failed".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_failures_preserve_status_without_forwarding_transport_text() {
        assert_eq!(
            info_request_error(ureq::Error::StatusCode(429)),
            ProviderError::RateLimited {
                retry_after_ms: None
            }
        );
        assert_eq!(
            info_request_error(ureq::Error::StatusCode(500)),
            ProviderError::Refused("hyperliquid info request failed: HTTP 500".to_string())
        );
        let error = std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "private proxy configuration",
        );
        assert_eq!(
            info_request_error(ureq::Error::Io(error)),
            ProviderError::Transport(
                "hyperliquid info transport failed: ConnectionReset".to_string()
            )
        );
        assert_eq!(
            info_request_error(ureq::Error::BadUri("private URL".to_string())),
            ProviderError::Transport("hyperliquid info request failed".to_string())
        );
    }
}
