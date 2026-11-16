//! Binary stream frames: the only backend-to-UI market-data encoding.
//!
//! Every frame is `[version u8][kind u8][key_len u16][key][generation u64]`
//! followed by a kind-specific payload, all little-endian. Book and bar
//! streams open with a snapshot and continue with deltas; a consumer that
//! falls behind receives a fresh snapshot. Absent optional integers use
//! sentinels: `i64::MIN` for an absent quantity, `u32::MAX` for an absent
//! order count, quantity zero for a delta removal.

use crate::views::SessionStateView;
use aeris_domain::{
    AggressorTradeVolumes, ChartInterval, DepthLevel, MarketBar, OrderBookPublication,
    OrderBookState, QuoteLevel,
};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: u8 = 1;
pub const KIND_BOOK_SNAPSHOT: u8 = 1;
pub const KIND_BOOK_DELTA: u8 = 2;
pub const KIND_BARS_SNAPSHOT: u8 = 3;
pub const KIND_BARS_UPDATE: u8 = 4;
pub const KIND_SESSION: u8 = 5;

/// Stream key for one provider's session lifecycle.
#[must_use]
pub fn session_stream_key(provider_id: &str) -> String {
    format!("session:{provider_id}")
}

/// Stream key for one instrument's book.
#[must_use]
pub fn book_stream_key(instrument_id: &str) -> String {
    format!("book:{instrument_id}")
}

/// Stream key for one instrument's candle series.
#[must_use]
pub fn bars_stream_key(instrument_id: &str, interval: ChartInterval) -> String {
    format!("bars:{instrument_id}:{}", interval.label())
}

struct Writer(Vec<u8>);

impl Writer {
    fn new() -> Self {
        Self(Vec::with_capacity(256))
    }
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn i64(&mut self, value: i64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn text(&mut self, value: &str) {
        self.u16(u16::try_from(value.len()).unwrap_or(u16::MAX));
        self.0.extend_from_slice(value.as_bytes());
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        if self.0.len() < count {
            return None;
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn text(&mut self) -> Option<&'a str> {
        let len = self.u16()? as usize;
        std::str::from_utf8(self.take(len)?).ok()
    }
}

fn frame(kind: u8, key: &str, generation: u64, payload: &[u8]) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u8(PROTOCOL_VERSION);
    writer.u8(kind);
    writer.text(key);
    writer.u64(generation);
    writer.0.extend_from_slice(payload);
    writer.0
}

/// One parsed frame header with its untouched payload.
pub struct Frame<'a> {
    pub kind: u8,
    pub key: &'a str,
    pub generation: u64,
    pub payload: &'a [u8],
}

/// Parses a frame header, rejecting unknown versions and truncated input.
#[must_use]
pub fn parse_frame(bytes: &[u8]) -> Option<Frame<'_>> {
    let mut reader = Reader(bytes);
    if reader.u8()? != PROTOCOL_VERSION {
        return None;
    }
    let kind = reader.u8()?;
    let key = reader.text()?;
    let generation = reader.u64()?;
    Some(Frame {
        kind,
        key,
        generation,
        payload: reader.0,
    })
}

fn state_bytes(state: &OrderBookState) -> (u8, u8) {
    match state {
        OrderBookState::Recovering(reason) => (
            0,
            match reason {
                aeris_domain::OrderBookRecoveryReason::AwaitingSnapshot => 0,
                aeris_domain::OrderBookRecoveryReason::SequenceGap => 1,
                aeris_domain::OrderBookRecoveryReason::CrossedBook => 2,
                aeris_domain::OrderBookRecoveryReason::InvalidUpdate => 3,
            },
        ),
        OrderBookState::Ready => (1, 0),
        OrderBookState::Stale => (2, 0),
    }
}

fn write_quote(writer: &mut Writer, quote: QuoteLevel) {
    writer.i64(quote.price);
    writer.i64(quote.quantity.unwrap_or(i64::MIN));
    writer.u32(quote.order_count.unwrap_or(u32::MAX));
}

fn write_level(writer: &mut Writer, level: DepthLevel) {
    writer.i64(level.price);
    writer.i64(level.quantity);
    writer.u32(level.order_count.unwrap_or(u32::MAX));
}

fn write_book_head(writer: &mut Writer, image: &OrderBookPublication) {
    writer.u64(image.revision);
    writer.u64(image.source_watermark);
    let (state, reason) = state_bytes(&image.state);
    writer.u8(state);
    writer.u8(reason);
    let mut flags = u8::from(image.top_of_book_only);
    if image.best_bid.is_some() {
        flags |= 0b10;
    }
    if image.best_ask.is_some() {
        flags |= 0b100;
    }
    writer.u8(flags);
    if let Some(bid) = image.best_bid {
        write_quote(writer, bid);
    }
    if let Some(ask) = image.best_ask {
        write_quote(writer, ask);
    }
    writer.u64(image.bbo_source_watermark);
    writer.u64(image.trade_source_watermark);
}

/// Encodes one complete book image as a snapshot frame.
#[must_use]
pub fn book_snapshot_frame(image: &OrderBookPublication) -> Vec<u8> {
    let mut writer = Writer::new();
    write_book_head(&mut writer, image);
    writer.u16(u16::try_from(image.bids.len()).unwrap_or(u16::MAX));
    writer.u16(u16::try_from(image.asks.len()).unwrap_or(u16::MAX));
    for level in image.bids.iter().chain(image.asks.iter()) {
        write_level(&mut writer, *level);
    }
    writer.u16(u16::try_from(image.traded_volumes.len()).unwrap_or(u16::MAX));
    for (price, volume) in &image.traded_volumes {
        writer.i64(*price);
        writer.i64(volume.buy);
        writer.i64(volume.sell);
    }
    frame(
        KIND_BOOK_SNAPSHOT,
        &book_stream_key(&image.instrument_id),
        image.session_generation,
        &writer.0,
    )
}

fn level_changes(current: &[DepthLevel], previous: &[DepthLevel]) -> Vec<DepthLevel> {
    let before: BTreeMap<i64, DepthLevel> =
        previous.iter().map(|level| (level.price, *level)).collect();
    let mut changes = Vec::new();
    for level in current {
        if before.get(&level.price) != Some(level) {
            changes.push(*level);
        }
    }
    for price in before.keys() {
        if !current.iter().any(|level| level.price == *price) {
            changes.push(DepthLevel {
                price: *price,
                quantity: 0,
                order_count: None,
            });
        }
    }
    changes
}

fn volume_changes(
    current: &BTreeMap<i64, AggressorTradeVolumes>,
    previous: &BTreeMap<i64, AggressorTradeVolumes>,
) -> Vec<(i64, i64, i64)> {
    let mut changes = Vec::new();
    for (price, volume) in current {
        if previous.get(price) != Some(volume) {
            changes.push((*price, volume.buy, volume.sell));
        }
    }
    for price in previous.keys() {
        if !current.contains_key(price) {
            changes.push((*price, 0, 0));
        }
    }
    changes
}

/// Encodes the difference between two book images as a delta frame. The
/// caller sends a snapshot instead when generations differ or the consumer
/// fell behind.
#[must_use]
pub fn book_delta_frame(
    current: &OrderBookPublication,
    previous: &OrderBookPublication,
) -> Vec<u8> {
    let mut writer = Writer::new();
    write_book_head(&mut writer, current);
    let bids = level_changes(&current.bids, &previous.bids);
    let asks = level_changes(&current.asks, &previous.asks);
    writer.u16(u16::try_from(bids.len()).unwrap_or(u16::MAX));
    writer.u16(u16::try_from(asks.len()).unwrap_or(u16::MAX));
    for level in bids.iter().chain(asks.iter()) {
        write_level(&mut writer, *level);
    }
    let volumes = volume_changes(&current.traded_volumes, &previous.traded_volumes);
    writer.u16(u16::try_from(volumes.len()).unwrap_or(u16::MAX));
    for (price, buy, sell) in volumes {
        writer.i64(price);
        writer.i64(buy);
        writer.i64(sell);
    }
    frame(
        KIND_BOOK_DELTA,
        &book_stream_key(&current.instrument_id),
        current.session_generation,
        &writer.0,
    )
}

fn write_bar(writer: &mut Writer, bar: &MarketBar) {
    writer.u64(bar.source_sequence);
    writer.i64(bar.exchange_timestamp_unix_nanos);
    writer.i64(bar.open);
    writer.i64(bar.high);
    writer.i64(bar.low);
    writer.i64(bar.close);
    writer.i64(bar.volume);
}

/// Encodes closed history plus the forming candle as a bars snapshot frame.
#[must_use]
pub fn bars_snapshot_frame(
    instrument_id: &str,
    interval: ChartInterval,
    generation: u64,
    closed: &[MarketBar],
    forming: Option<MarketBar>,
) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u32(u32::try_from(closed.len()).unwrap_or(u32::MAX));
    for bar in closed {
        write_bar(&mut writer, bar);
    }
    writer.u8(u8::from(forming.is_some()));
    if let Some(bar) = forming {
        write_bar(&mut writer, &bar);
    }
    frame(
        KIND_BARS_SNAPSHOT,
        &bars_stream_key(instrument_id, interval),
        generation,
        &writer.0,
    )
}

/// Encodes completed-bar appends and a forming replacement (or clear) as a
/// bars update frame.
#[must_use]
pub fn bars_update_frame(
    instrument_id: &str,
    interval: ChartInterval,
    generation: u64,
    completed: &[MarketBar],
    forming: Option<MarketBar>,
    clear_forming: bool,
) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u32(u32::try_from(completed.len()).unwrap_or(u32::MAX));
    for bar in completed {
        write_bar(&mut writer, bar);
    }
    let flag = if clear_forming {
        2
    } else {
        u8::from(forming.is_some())
    };
    writer.u8(flag);
    if let Some(bar) = forming {
        write_bar(&mut writer, &bar);
    }
    frame(
        KIND_BARS_UPDATE,
        &bars_stream_key(instrument_id, interval),
        generation,
        &writer.0,
    )
}

/// Encodes session lifecycle evidence as a session frame.
#[must_use]
pub fn session_frame(provider_id: &str, generation: u64, state: &SessionStateView) -> Vec<u8> {
    let mut writer = Writer::new();
    let (tag, attempt, message) = match state {
        SessionStateView::Connecting => (0, 0, ""),
        SessionStateView::Live => (1, 0, ""),
        SessionStateView::Reconnecting { attempt } => (2, *attempt, ""),
        SessionStateView::Failed { message } => (3, 0, message.as_str()),
        SessionStateView::Stopped => (4, 0, ""),
    };
    writer.u8(tag);
    writer.u32(attempt);
    writer.text(message);
    frame(
        KIND_SESSION,
        &session_stream_key(provider_id),
        generation,
        &writer.0,
    )
}

/// One decoded level; quantity zero in a delta means removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedLevel {
    pub price: i64,
    pub quantity: i64,
    pub order_count: Option<u32>,
}

/// One decoded BBO side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedQuote {
    pub price: i64,
    pub quantity: Option<i64>,
    pub order_count: Option<u32>,
}

/// Book evidence shared by snapshot and delta payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookHead {
    pub revision: u64,
    pub source_watermark: u64,
    pub state: u8,
    pub reason: u8,
    pub top_of_book_only: bool,
    pub best_bid: Option<DecodedQuote>,
    pub best_ask: Option<DecodedQuote>,
    pub bbo_source_watermark: u64,
    pub trade_source_watermark: u64,
}

/// A decoded complete book image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBookSnapshot {
    pub head: BookHead,
    pub bids: Vec<DecodedLevel>,
    pub asks: Vec<DecodedLevel>,
    pub volumes: Vec<(i64, i64, i64)>,
}

/// A decoded book difference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBookDelta {
    pub head: BookHead,
    pub bid_changes: Vec<DecodedLevel>,
    pub ask_changes: Vec<DecodedLevel>,
    pub volume_changes: Vec<(i64, i64, i64)>,
}

/// One decoded bar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedBar {
    pub sequence: u64,
    pub open_unix_nanos: i64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub volume: i64,
}

/// A decoded bars snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBarsSnapshot {
    pub closed: Vec<DecodedBar>,
    pub forming: Option<DecodedBar>,
}

/// A decoded bars update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBarsUpdate {
    pub completed: Vec<DecodedBar>,
    pub forming: Option<DecodedBar>,
    pub clear_forming: bool,
}

/// Decoded session evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSession {
    pub state: u8,
    pub attempt: u32,
    pub message: String,
}

fn read_quote(reader: &mut Reader<'_>) -> Option<DecodedQuote> {
    let price = reader.i64()?;
    let quantity = reader.i64()?;
    let order_count = reader.u32()?;
    Some(DecodedQuote {
        price,
        quantity: (quantity != i64::MIN).then_some(quantity),
        order_count: (order_count != u32::MAX).then_some(order_count),
    })
}

fn read_level(reader: &mut Reader<'_>) -> Option<DecodedLevel> {
    let price = reader.i64()?;
    let quantity = reader.i64()?;
    let order_count = reader.u32()?;
    Some(DecodedLevel {
        price,
        quantity,
        order_count: (order_count != u32::MAX).then_some(order_count),
    })
}

fn read_book_head(reader: &mut Reader<'_>) -> Option<BookHead> {
    let revision = reader.u64()?;
    let source_watermark = reader.u64()?;
    let state = reader.u8()?;
    let reason = reader.u8()?;
    let flags = reader.u8()?;
    let best_bid = (flags & 0b10 != 0).then(|| read_quote(reader)).flatten();
    let best_ask = (flags & 0b100 != 0).then(|| read_quote(reader)).flatten();
    Some(BookHead {
        revision,
        source_watermark,
        state,
        reason,
        top_of_book_only: flags & 1 != 0,
        best_bid,
        best_ask,
        bbo_source_watermark: reader.u64()?,
        trade_source_watermark: reader.u64()?,
    })
}

fn read_levels(reader: &mut Reader<'_>, count: usize) -> Option<Vec<DecodedLevel>> {
    (0..count).map(|_| read_level(reader)).collect()
}

fn read_volumes(reader: &mut Reader<'_>) -> Option<Vec<(i64, i64, i64)>> {
    let count = reader.u16()? as usize;
    (0..count)
        .map(|_| Some((reader.i64()?, reader.i64()?, reader.i64()?)))
        .collect()
}

/// Decodes a book snapshot payload.
#[must_use]
pub fn decode_book_snapshot(payload: &[u8]) -> Option<DecodedBookSnapshot> {
    let mut reader = Reader(payload);
    let head = read_book_head(&mut reader)?;
    let bid_count = reader.u16()? as usize;
    let ask_count = reader.u16()? as usize;
    let bids = read_levels(&mut reader, bid_count)?;
    let asks = read_levels(&mut reader, ask_count)?;
    Some(DecodedBookSnapshot {
        head,
        bids,
        asks,
        volumes: read_volumes(&mut reader)?,
    })
}

/// Decodes a book delta payload.
#[must_use]
pub fn decode_book_delta(payload: &[u8]) -> Option<DecodedBookDelta> {
    let mut reader = Reader(payload);
    let head = read_book_head(&mut reader)?;
    let bid_count = reader.u16()? as usize;
    let ask_count = reader.u16()? as usize;
    let bid_changes = read_levels(&mut reader, bid_count)?;
    let ask_changes = read_levels(&mut reader, ask_count)?;
    Some(DecodedBookDelta {
        head,
        bid_changes,
        ask_changes,
        volume_changes: read_volumes(&mut reader)?,
    })
}

fn read_bar(reader: &mut Reader<'_>) -> Option<DecodedBar> {
    Some(DecodedBar {
        sequence: reader.u64()?,
        open_unix_nanos: reader.i64()?,
        open: reader.i64()?,
        high: reader.i64()?,
        low: reader.i64()?,
        close: reader.i64()?,
        volume: reader.i64()?,
    })
}

fn read_bars(reader: &mut Reader<'_>, count: usize) -> Option<Vec<DecodedBar>> {
    (0..count).map(|_| read_bar(reader)).collect()
}

/// Decodes a bars snapshot payload.
#[must_use]
pub fn decode_bars_snapshot(payload: &[u8]) -> Option<DecodedBarsSnapshot> {
    let mut reader = Reader(payload);
    let count = reader.u32()? as usize;
    let closed = read_bars(&mut reader, count)?;
    let flag = reader.u8()?;
    let forming = (flag == 1).then(|| read_bar(&mut reader)).flatten();
    Some(DecodedBarsSnapshot { closed, forming })
}

/// Decodes a bars update payload.
#[must_use]
pub fn decode_bars_update(payload: &[u8]) -> Option<DecodedBarsUpdate> {
    let mut reader = Reader(payload);
    let count = reader.u32()? as usize;
    let completed = read_bars(&mut reader, count)?;
    let flag = reader.u8()?;
    let forming = (flag == 1).then(|| read_bar(&mut reader)).flatten();
    Some(DecodedBarsUpdate {
        completed,
        forming,
        clear_forming: flag == 2,
    })
}

/// Decodes a session payload.
#[must_use]
pub fn decode_session(payload: &[u8]) -> Option<DecodedSession> {
    let mut reader = Reader(payload);
    Some(DecodedSession {
        state: reader.u8()?,
        attempt: reader.u32()?,
        message: reader.text()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeris_domain::{OrderBookRecoveryReason, QuoteLevel};

    fn image() -> OrderBookPublication {
        OrderBookPublication {
            provider_id: "fixture".to_string(),
            instrument_id: "instrument:fixture:BTC".to_string(),
            entitlement_id: "fixture-public".to_string(),
            session_generation: 3,
            revision: 9,
            source_watermark: 40,
            best_bid: Some(QuoteLevel {
                price: 100,
                quantity: Some(3),
                order_count: None,
            }),
            best_ask: Some(QuoteLevel {
                price: 102,
                quantity: None,
                order_count: Some(2),
            }),
            bbo_source_watermark: 7,
            bids: vec![
                DepthLevel {
                    price: 100,
                    quantity: 3,
                    order_count: None,
                },
                DepthLevel {
                    price: 99,
                    quantity: 5,
                    order_count: Some(1),
                },
            ],
            asks: vec![DepthLevel {
                price: 102,
                quantity: 4,
                order_count: None,
            }],
            traded_volumes: [(100, AggressorTradeVolumes { buy: 11, sell: 7 })]
                .into_iter()
                .collect(),
            trade_source_watermark: 12,
            top_of_book_only: false,
            state: OrderBookState::Ready,
        }
    }

    fn bar(sequence: u64, open_nanos: i64) -> MarketBar {
        MarketBar {
            source_sequence: sequence,
            exchange_timestamp_seconds: open_nanos / 1_000_000_000,
            exchange_timestamp_unix_nanos: open_nanos,
            open: 100,
            high: 110,
            low: 90,
            close: 105,
            volume: 10,
        }
    }

    #[test]
    fn book_snapshot_round_trips_every_field() {
        let original = image();
        let bytes = book_snapshot_frame(&original);
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_SNAPSHOT);
        assert_eq!(frame.key, "book:instrument:fixture:BTC");
        assert_eq!(frame.generation, 3);
        let decoded = decode_book_snapshot(frame.payload).expect("snapshot");
        assert_eq!(decoded.head.revision, 9);
        assert_eq!(decoded.head.state, 1);
        assert_eq!(decoded.head.best_bid.expect("bid").quantity, Some(3));
        assert_eq!(decoded.head.best_ask.expect("ask").quantity, None);
        assert_eq!(decoded.head.best_ask.expect("ask").order_count, Some(2));
        assert_eq!(decoded.bids.len(), 2);
        assert_eq!(decoded.asks.len(), 1);
        assert_eq!(decoded.volumes, vec![(100, 11, 7)]);
    }

    #[test]
    fn book_delta_carries_only_changes_and_removals() {
        let previous = image();
        let mut current = image();
        current.revision = 10;
        current.source_watermark = 41;
        current.bids[0].quantity = 8;
        current.bids.remove(1);
        current.asks.push(DepthLevel {
            price: 103,
            quantity: 2,
            order_count: None,
        });
        current.traded_volumes.remove(&100);
        let bytes = book_delta_frame(&current, &previous);
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_DELTA);
        let decoded = decode_book_delta(frame.payload).expect("delta");
        assert_eq!(decoded.bid_changes.len(), 2);
        assert!(
            decoded
                .bid_changes
                .iter()
                .any(|level| level.price == 100 && level.quantity == 8)
        );
        assert!(
            decoded
                .bid_changes
                .iter()
                .any(|level| level.price == 99 && level.quantity == 0)
        );
        assert_eq!(decoded.ask_changes.len(), 1);
        assert_eq!(decoded.ask_changes[0].price, 103);
        assert_eq!(decoded.volume_changes, vec![(100, 0, 0)]);
    }

    #[test]
    fn bars_frames_round_trip_history_and_forming() {
        let closed = vec![bar(1, 60_000_000_000), bar(2, 120_000_000_000)];
        let forming = bar(3, 180_000_000_000);
        let bytes = bars_snapshot_frame(
            "instrument:fixture:BTC",
            ChartInterval::Minute1,
            3,
            &closed,
            Some(forming),
        );
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.key, "bars:instrument:fixture:BTC:1m");
        let decoded = decode_bars_snapshot(frame.payload).expect("snapshot");
        assert_eq!(decoded.closed.len(), 2);
        assert_eq!(decoded.closed[1].sequence, 2);
        assert_eq!(decoded.forming.expect("forming").close, 105);

        let bytes = bars_update_frame(
            "instrument:fixture:BTC",
            ChartInterval::Minute1,
            3,
            &[forming],
            Some(bar(4, 240_000_000_000)),
            false,
        );
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BARS_UPDATE);
        let decoded = decode_bars_update(frame.payload).expect("update");
        assert_eq!(decoded.completed.len(), 1);
        assert_eq!(decoded.forming.expect("forming").sequence, 4);
        assert!(!decoded.clear_forming);
    }

    #[test]
    fn session_frame_matches_the_shared_fixture() {
        // The TypeScript decoder tests assert against these same bytes.
        let bytes = session_frame("fixture", 1, &SessionStateView::Reconnecting { attempt: 1 });
        assert_eq!(
            bytes,
            vec![
                0x01, 0x05, 0x0f, 0x00, b's', b'e', b's', b's', b'i', b'o', b'n', b':', b'f', b'i',
                b'x', b't', b'u', b'r', b'e', 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]
        );
        let frame = parse_frame(&bytes).expect("frame");
        let decoded = decode_session(frame.payload).expect("session");
        assert_eq!(decoded.state, 2);
        assert_eq!(decoded.attempt, 1);
        assert_eq!(decoded.message, "");
    }

    #[test]
    fn truncated_and_unknown_frames_fail_closed() {
        assert!(parse_frame(&[]).is_none());
        assert!(parse_frame(&[0xFF]).is_none());
        let bytes = book_snapshot_frame(&image());
        assert!(parse_frame(&bytes[..bytes.len() - 1]).is_some());
        assert!(decode_book_snapshot(&bytes[..bytes.len() - 1]).is_none());
        let recovering = OrderBookState::Recovering(OrderBookRecoveryReason::SequenceGap);
        let (state, reason) = state_bytes(&recovering);
        assert_eq!((state, reason), (0, 1));
    }
}
