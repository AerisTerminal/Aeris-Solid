//! Typed UI commands: intent in, typed result or typed error out. Commands
//! never carry an authoritative price.

use crate::interval::IntervalLabel;
use crate::views::{BarView, InstrumentView, ProviderView};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One UI request to the composition root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../src/protocol/")]
pub enum Command {
    /// List registered providers and their declarations.
    ListProviders,
    /// Search instruments across registered catalogs.
    SearchInstruments { query: String, limit: u32 },
    /// Open the market stream for one instrument: book, trades, quotes and
    /// live candles at the requested interval.
    Subscribe {
        instrument_id: String,
        interval: IntervalLabel,
    },
    /// Close one instrument's market stream.
    Unsubscribe { instrument_id: String },
    /// Fetch closed history without opening a stream.
    History {
        instrument_id: String,
        interval: IntervalLabel,
        limit: u32,
    },
}

/// The typed answer to one command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "result",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../src/protocol/")]
pub enum CommandResult {
    Providers { providers: Vec<ProviderView> },
    Instruments { instruments: Vec<InstrumentView> },
    History { bars: Vec<BarView> },
    Subscribed { instrument_id: String },
    Unsubscribed { instrument_id: String },
}

/// Why a command failed. The UI branches on the kind, never on the text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub enum CommandErrorKind {
    /// The request itself is malformed or names something unknown.
    InvalidRequest,
    /// No registered catalog knows this instrument.
    UnknownInstrument,
    /// The provider does not offer this interval.
    UnsupportedInterval,
    /// The provider could not be reached or refused the request.
    ProviderUnavailable,
    /// The provider throttled the request; retry later.
    RateLimited,
    /// An unexpected internal failure.
    Internal,
}

/// One typed command failure with user-facing text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/protocol/")]
pub struct CommandError {
    pub kind: CommandErrorKind,
    pub message: String,
}

impl CommandError {
    /// Builds one typed failure.
    #[must_use]
    pub fn new(kind: CommandErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_round_trip_through_camel_case_json() {
        let command = Command::Subscribe {
            instrument_id: "instrument:fixture:BTC".to_string(),
            interval: IntervalLabel("1m".to_string()),
        };
        let json = serde_json::to_string(&command).expect("serialize");
        assert_eq!(
            json,
            r#"{"command":"subscribe","instrumentId":"instrument:fixture:BTC","interval":"1m"}"#
        );
        let back: Command = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, command);
    }

    #[test]
    fn errors_carry_a_kind_the_ui_can_branch_on() {
        let error = CommandError::new(CommandErrorKind::UnknownInstrument, "not listed");
        let json = serde_json::to_string(&error).expect("serialize");
        let back: CommandError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.kind, CommandErrorKind::UnknownInstrument);
    }

    #[test]
    fn results_round_trip_through_camel_case_json() {
        let result = CommandResult::Providers {
            providers: Vec::new(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert_eq!(json, r#"{"result":"providers","providers":[]}"#);
        let back: CommandResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, result);

        let result = CommandResult::Subscribed {
            instrument_id: "instrument:fixture:BTC".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert_eq!(
            json,
            r#"{"result":"subscribed","instrumentId":"instrument:fixture:BTC"}"#
        );
        let back: CommandResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, result);
    }
}
