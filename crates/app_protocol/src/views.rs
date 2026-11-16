//! UI-facing value shapes. Fixed-point integers travel as decimal strings in
//! JSON so JavaScript numbers never round them; binary frames carry true
//! 64-bit integers.

use crate::interval::IntervalLabel;
use aeris_domain::{InstrumentDescriptor, MarketBar};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

mod i64_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

mod opt_i64_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<i64>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        text.map(|text| text.parse().map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// One instrument as the UI may know it: identity, display and scales.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct InstrumentView {
    pub instrument_id: String,
    pub symbol: String,
    pub display: String,
    pub exchange: String,
    pub asset_class: String,
    pub price_scale: u8,
    pub quantity_scale: u8,
    #[serde(with = "opt_i64_text")]
    #[ts(type = "string | null")]
    pub price_increment: Option<i64>,
    #[serde(with = "opt_i64_text")]
    #[ts(type = "string | null")]
    pub quantity_increment: Option<i64>,
}

impl From<&InstrumentDescriptor> for InstrumentView {
    fn from(descriptor: &InstrumentDescriptor) -> Self {
        Self {
            instrument_id: descriptor.instrument_id.as_str().to_string(),
            symbol: descriptor.symbol.clone(),
            display: descriptor.display.clone(),
            exchange: descriptor.exchange.clone(),
            asset_class: asset_class_label(descriptor.asset_class).to_string(),
            price_scale: descriptor.price_scale,
            quantity_scale: descriptor.quantity_scale,
            price_increment: descriptor.price_increment,
            quantity_increment: descriptor.quantity_increment,
        }
    }
}

/// One closed or forming bar on the provider's history clock.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct BarView {
    pub sequence: u64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub open_unix_nanos: i64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub open: i64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub high: i64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub low: i64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub close: i64,
    #[serde(with = "i64_text")]
    #[ts(type = "string")]
    pub volume: i64,
}

impl From<&MarketBar> for BarView {
    fn from(bar: &MarketBar) -> Self {
        Self {
            sequence: bar.source_sequence,
            open_unix_nanos: bar.exchange_timestamp_unix_nanos,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        }
    }
}

/// Stream surface a provider declares, with its bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct StreamsView {
    pub trades: bool,
    pub quotes: bool,
    pub aggregate_depth_levels: Option<u32>,
    pub order_level_instrument_cap: Option<u32>,
}

/// The provider's request-weight window, as declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct RateBudgetView {
    pub window_seconds: u32,
    pub window_weight: u32,
    pub foreground_reserve: u32,
}

/// Build targets one provider supports.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct TargetsView {
    pub native: bool,
    pub web: bool,
}

/// One registered provider's declaration, without any provider-specific code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct ProviderView {
    pub provider_id: String,
    pub display_name: String,
    pub asset_classes: Vec<String>,
    pub streams: StreamsView,
    pub history_intervals: Vec<IntervalLabel>,
    pub sign_in: String,
    pub trading_venue: bool,
    pub rate_budget: RateBudgetView,
    pub targets: TargetsView,
}

/// Session lifecycle evidence for the UI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub enum SessionStateView {
    Connecting,
    Live,
    Reconnecting { attempt: u32 },
    Failed { message: String },
    Stopped,
}

/// Canonical asset-class labels shared by every view.
#[must_use]
pub const fn asset_class_label(class: aeris_domain::AssetClass) -> &'static str {
    match class {
        aeris_domain::AssetClass::CryptoPerp => "cryptoPerp",
        aeris_domain::AssetClass::CryptoSpot => "cryptoSpot",
        aeris_domain::AssetClass::Futures => "futures",
        aeris_domain::AssetClass::Stock => "stock",
        aeris_domain::AssetClass::Fx => "fx",
        aeris_domain::AssetClass::Option => "option",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_values_survive_json_as_exact_text() {
        let bar = BarView {
            sequence: 7,
            open_unix_nanos: 1_700_000_000_123_456_789,
            open: 9_007_199_254_740_993,
            high: 9_007_199_254_740_994,
            low: 9_007_199_254_740_992,
            close: 9_007_199_254_740_993,
            volume: i64::MAX,
        };
        let json = serde_json::to_string(&bar).expect("serialize");
        let back: BarView = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, bar);
        assert!(json.contains("\"9007199254740993\""));
    }
}
