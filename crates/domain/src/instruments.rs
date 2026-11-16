//! Instrument identity and reference data shared by every runtime.

use crate::{MAXIMUM_MARKET_DATA_FIELD_BYTES, MarketDataValidationError};
use core::fmt;

/// Largest fixed-point decimal scale an instrument may declare.
pub const MAXIMUM_DECIMAL_SCALE: u8 = 18;

/// Validated instrument identity, unique across providers.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstrumentId(String);

impl InstrumentId {
    /// Wraps a non-empty identity within the canonical field bound.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty or oversized identity.
    pub fn new(value: impl Into<String>) -> Result<Self, MarketDataValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MarketDataValidationError::EmptyField("instrument_id"));
        }
        if value.len() > MAXIMUM_MARKET_DATA_FIELD_BYTES {
            return Err(MarketDataValidationError::FieldTooLong {
                field: "instrument_id",
                maximum: MAXIMUM_MARKET_DATA_FIELD_BYTES,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Asset class declared by a provider capability, never inferred from a name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AssetClass {
    CryptoPerp,
    CryptoSpot,
    Futures,
    Stock,
    Fx,
    Option,
}

/// Versioned reference data for one tradable instrument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstrumentDescriptor {
    pub instrument_id: InstrumentId,
    pub provider_id: String,
    /// The provider's wire symbol, kept apart from canonical identity.
    pub symbol: String,
    /// Human-facing label, e.g. `BTC-USDC`.
    pub display: String,
    /// Presentation venue or exchange name.
    pub exchange: String,
    pub asset_class: AssetClass,
    /// Fixed-point scale: a stored value of `v` means `v * 10^-price_scale`.
    pub price_scale: u8,
    pub quantity_scale: u8,
    /// Authoritative minimum price increment at `price_scale`, when known.
    pub price_increment: Option<i64>,
    /// Authoritative minimum quantity increment at `quantity_scale`, when known.
    pub quantity_increment: Option<i64>,
}

impl InstrumentDescriptor {
    /// Validates identity, scales, and increments.
    ///
    /// # Errors
    ///
    /// Returns an error for empty identity fields, unsupported scales, or
    /// non-positive increments.
    pub fn validate(&self) -> Result<(), MarketDataValidationError> {
        for (field, value) in [
            ("provider_id", self.provider_id.as_str()),
            ("symbol", self.symbol.as_str()),
            ("display", self.display.as_str()),
            ("exchange", self.exchange.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketDataValidationError::EmptyField(field));
            }
            if value.len() > MAXIMUM_MARKET_DATA_FIELD_BYTES {
                return Err(MarketDataValidationError::FieldTooLong {
                    field,
                    maximum: MAXIMUM_MARKET_DATA_FIELD_BYTES,
                });
            }
        }
        if self.price_scale > MAXIMUM_DECIMAL_SCALE || self.quantity_scale > MAXIMUM_DECIMAL_SCALE {
            return Err(MarketDataValidationError::InvalidInstrument);
        }
        if self.price_increment.is_some_and(|tick| tick <= 0)
            || self.quantity_increment.is_some_and(|lot| lot <= 0)
        {
            return Err(MarketDataValidationError::InvalidInstrument);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> InstrumentDescriptor {
        InstrumentDescriptor {
            instrument_id: InstrumentId::new("instrument:fixture:BTC").expect("identity"),
            provider_id: "fixture".to_string(),
            symbol: "BTC".to_string(),
            display: "BTC-USDC".to_string(),
            exchange: "Fixture".to_string(),
            asset_class: AssetClass::CryptoPerp,
            price_scale: 8,
            quantity_scale: 8,
            price_increment: Some(100_000),
            quantity_increment: Some(1_000),
        }
    }

    #[test]
    fn instrument_identity_rejects_empty_and_oversized_values() {
        assert!(InstrumentId::new("  ").is_err());
        assert!(InstrumentId::new("x".repeat(MAXIMUM_MARKET_DATA_FIELD_BYTES + 1)).is_err());
        assert!(InstrumentId::new("instrument:fixture:BTC").is_ok());
    }

    #[test]
    fn descriptor_validation_covers_scales_and_increments() {
        let mut instrument = descriptor();
        assert_eq!(instrument.validate(), Ok(()));
        instrument.price_scale = MAXIMUM_DECIMAL_SCALE + 1;
        assert_eq!(
            instrument.validate(),
            Err(MarketDataValidationError::InvalidInstrument)
        );
        instrument.price_scale = 8;
        instrument.quantity_increment = Some(0);
        assert_eq!(
            instrument.validate(),
            Err(MarketDataValidationError::InvalidInstrument)
        );
        instrument.quantity_increment = None;
        instrument.display = String::new();
        assert_eq!(
            instrument.validate(),
            Err(MarketDataValidationError::EmptyField("display"))
        );
    }
}
