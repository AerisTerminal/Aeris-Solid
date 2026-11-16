//! Core perpetual catalog decoded from one `metaAndAssetCtxs` response.
//!
//! Three strings describe every market and stay apart: the stable canonical
//! `instrument_id`, the exact wire `coin` sent to the API, and the display
//! label. Perpetuals display as `BASE-USDC`; the wire coin is the universe
//! name itself.

use crate::decimal::{
    NORMALIZED_PRICE_SCALE, NORMALIZED_QUANTITY_SCALE, NOTIONAL_SCALE,
    parse_aggregate_decimal_to_fixed, price_increment_near,
};
use aeris_domain::{AssetClass, InstrumentDescriptor, InstrumentId};

/// One catalog market with its screening volume.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub instrument: InstrumentDescriptor,
    /// Day notional volume at [`NOTIONAL_SCALE`], zero when absent.
    pub day_notional: i64,
}

/// Decodes the `[meta, contexts]` response into validated catalog entries.
///
/// Contexts pair with their market by their own `coin` when every context
/// carries one, otherwise by universe position. A malformed or misaligned
/// response fails closed rather than reporting a partial catalog as complete.
///
/// # Errors
///
/// Returns an error for malformed metadata, invalid market identity or
/// precision, or inconsistent contexts.
pub fn decode_catalog(response: &serde_json::Value) -> Result<Vec<CatalogEntry>, String> {
    let parts = response
        .as_array()
        .filter(|parts| parts.len() == 2)
        .ok_or_else(|| "hyperliquid metadata and contexts are malformed".to_string())?;
    let universe = parts[0]
        .get("universe")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "hyperliquid metadata universe is malformed".to_string())?;
    let contexts = parts[1]
        .as_array()
        .ok_or_else(|| "hyperliquid asset contexts are malformed".to_string())?;
    let pairs = market_context_pairs(universe, contexts)?;
    let mut entries = Vec::with_capacity(pairs.len());
    for (market, context) in pairs {
        entries.push(decode_entry(market, context)?);
    }
    if entries.is_empty() {
        return Err("hyperliquid catalog is empty".to_string());
    }
    Ok(entries)
}

fn market_context_pairs<'a>(
    universe: &'a [serde_json::Value],
    contexts: &'a [serde_json::Value],
) -> Result<Vec<(&'a serde_json::Value, &'a serde_json::Value)>, String> {
    let contexts_carry_identity = contexts.iter().all(|context| {
        context
            .get("coin")
            .and_then(serde_json::Value::as_str)
            .is_some()
    });
    if contexts_carry_identity {
        let mut pairs = Vec::with_capacity(contexts.len());
        for context in contexts {
            let coin = context
                .get("coin")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let market = universe
                .iter()
                .find(|market| market.get("name").and_then(serde_json::Value::as_str) == Some(coin))
                .ok_or_else(|| "hyperliquid context names an unknown market".to_string())?;
            pairs.push((market, context));
        }
        return Ok(pairs);
    }
    if universe.len() != contexts.len() {
        return Err("hyperliquid metadata and contexts are inconsistent".to_string());
    }
    Ok(universe.iter().zip(contexts).collect())
}

fn decode_entry(
    market: &serde_json::Value,
    context: &serde_json::Value,
) -> Result<CatalogEntry, String> {
    let name = market
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "hyperliquid market name is missing".to_string())?;
    if name.trim().is_empty() || name.len() > 64 || name.contains(':') || name.contains('/') {
        return Err("hyperliquid market name is invalid".to_string());
    }
    let sz_decimals = u32::try_from(
        market
            .get("szDecimals")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "hyperliquid size decimals are missing".to_string())?,
    )
    .map_err(|_| "hyperliquid size decimals are invalid".to_string())?;
    if sz_decimals > 6 {
        return Err("hyperliquid size decimals exceed the price grid limit".to_string());
    }
    let price_increment = match context.get("markPx").and_then(serde_json::Value::as_str) {
        Some(mark) => Some(price_increment_near(mark, sz_decimals, false)?),
        None => None,
    };
    let day_notional = context
        .get("dayNtlVlm")
        .and_then(serde_json::Value::as_str)
        .and_then(|volume| parse_aggregate_decimal_to_fixed(volume, NOTIONAL_SCALE).ok())
        .unwrap_or(0);
    let instrument = InstrumentDescriptor {
        instrument_id: InstrumentId::new(format!("instrument:hyperliquid:{name}"))
            .map_err(|error| error.to_string())?,
        provider_id: crate::PROVIDER_ID.to_string(),
        symbol: name.to_string(),
        display: format!("{name}-USDC"),
        exchange: "Hyperliquid".to_string(),
        asset_class: AssetClass::CryptoPerp,
        price_scale: u8::try_from(NORMALIZED_PRICE_SCALE)
            .map_err(|_| "hyperliquid instrument precision is invalid".to_string())?,
        quantity_scale: u8::try_from(NORMALIZED_QUANTITY_SCALE)
            .map_err(|_| "hyperliquid instrument precision is invalid".to_string())?,
        price_increment,
        quantity_increment: Some(10_i64.pow(NORMALIZED_QUANTITY_SCALE - sz_decimals)),
    };
    instrument.validate().map_err(|error| error.to_string())?;
    Ok(CatalogEntry {
        instrument,
        day_notional,
    })
}

/// Searches entries by free text, leading with the highest day notional volume.
#[must_use]
pub fn search(entries: &[CatalogEntry], query: &str, limit: usize) -> Vec<InstrumentDescriptor> {
    let query = query.trim().to_lowercase();
    let mut matches: Vec<&CatalogEntry> = entries
        .iter()
        .filter(|entry| {
            query.is_empty()
                || entry.instrument.symbol.to_lowercase().contains(&query)
                || entry.instrument.display.to_lowercase().contains(&query)
        })
        .collect();
    matches.sort_by(|left, right| {
        right
            .day_notional
            .cmp(&left.day_notional)
            .then_with(|| left.instrument.symbol.cmp(&right.instrument.symbol))
    });
    matches
        .into_iter()
        .take(limit)
        .map(|entry| entry.instrument.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response() -> serde_json::Value {
        json!([
            {"universe": [
                {"name": "BTC", "szDecimals": 5},
                {"name": "ETH", "szDecimals": 4},
                {"name": "PURR", "szDecimals": 0},
            ]},
            [
                {"markPx": "85031.0", "dayNtlVlm": "785361336.8372405767"},
                {"markPx": "3012.4", "dayNtlVlm": "1000000.0"},
                {"dayNtlVlm": "50.0"},
            ],
        ])
    }

    #[test]
    fn catalog_decodes_identity_increments_and_volume() {
        let entries = decode_catalog(&response()).expect("catalog");
        assert_eq!(entries.len(), 3);
        let btc = &entries[0];
        assert_eq!(
            btc.instrument.instrument_id.as_str(),
            "instrument:hyperliquid:BTC"
        );
        assert_eq!(btc.instrument.symbol, "BTC");
        assert_eq!(btc.instrument.display, "BTC-USDC");
        assert_eq!(btc.instrument.price_increment, Some(100_000_000));
        assert_eq!(btc.instrument.quantity_increment, Some(1_000));
        assert_eq!(btc.day_notional, 7_853_613_368_372_405_767);
        assert_eq!(entries[1].instrument.price_increment, Some(10_000_000));
        assert_eq!(
            entries[2].instrument.price_increment, None,
            "a context without a mark price leaves the increment unknown"
        );
    }

    #[test]
    fn search_leads_with_volume_and_matches_symbol_or_display() {
        let entries = decode_catalog(&response()).expect("catalog");
        let leading = search(&entries, "", 2);
        assert_eq!(leading.len(), 2);
        assert_eq!(leading[0].symbol, "BTC");
        assert_eq!(leading[1].symbol, "ETH");
        let found = search(&entries, "eth-usdc", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol, "ETH");
        assert!(search(&entries, "DOGE", 10).is_empty());
    }

    #[test]
    fn contexts_pair_by_own_coin_when_present() {
        let response = json!([
            {"universe": [{"name": "BTC", "szDecimals": 5}, {"name": "ETH", "szDecimals": 4}]},
            [{"coin": "ETH", "markPx": "3012.4"}, {"coin": "BTC", "markPx": "85031.0"}],
        ]);
        let entries = decode_catalog(&response).expect("catalog");
        assert_eq!(entries[0].instrument.symbol, "ETH");
        assert_eq!(entries[1].instrument.symbol, "BTC");
    }

    #[test]
    fn malformed_and_misaligned_metadata_fail_closed() {
        assert!(decode_catalog(&json!({})).is_err());
        assert!(decode_catalog(&json!([{"universe": []}, []])).is_err());
        let misaligned = json!([
            {"universe": [{"name": "BTC", "szDecimals": 5}]},
            [],
        ]);
        assert!(decode_catalog(&misaligned).is_err());
        let bad_precision = json!([
            {"universe": [{"name": "BTC", "szDecimals": 7}]},
            [{"markPx": "1.0"}],
        ]);
        assert!(decode_catalog(&bad_precision).is_err());
        let unknown_coin = json!([
            {"universe": [{"name": "BTC", "szDecimals": 5}]},
            [{"coin": "ETH", "markPx": "1.0"}],
        ]);
        assert!(decode_catalog(&unknown_coin).is_err());
    }
}
