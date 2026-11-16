//! Every message between backend and UI: typed commands and results, value
//! views, and the binary stream frames. The TypeScript types in `src/protocol`
//! are generated from this crate by `cargo test -p aeris_app_protocol` and
//! are never edited by hand.

pub mod binary;
pub mod commands;
pub mod interval;
pub mod views;

pub use commands::{Command, CommandError, CommandErrorKind, CommandResult};
pub use interval::IntervalLabel;
pub use views::{
    BarView, InstrumentView, ProviderView, RateBudgetView, SessionStateView, StreamsView,
    TargetsView, asset_class_label,
};
