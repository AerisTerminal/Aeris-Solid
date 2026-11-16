//! Wire interval labels: the chart ladder's own labels, validated at the
//! command boundary so generic code never parses provider formats.

use aeris_domain::ChartInterval;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One validated chart-interval label, e.g. `1m` or `1M`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../../../src/protocol/",
    type = "\"100t\" | \"1s\" | \"5s\" | \"10s\" | \"15s\" | \"30s\" | \"1m\" | \"3m\" | \"5m\" | \"15m\" | \"30m\" | \"1h\" | \"2h\" | \"4h\" | \"8h\" | \"12h\" | \"1D\" | \"3D\" | \"1W\" | \"1M\""
)]
pub struct IntervalLabel(pub String);

impl IntervalLabel {
    /// Parses a label into the canonical interval, rejecting unknown text.
    #[must_use]
    pub fn parse(&self) -> Option<ChartInterval> {
        ChartInterval::ALL
            .into_iter()
            .find(|interval| interval.label() == self.0)
    }

    /// The canonical label for one interval.
    #[must_use]
    pub fn of(interval: ChartInterval) -> Self {
        Self(interval.label().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_interval_round_trips_through_its_label() {
        for interval in ChartInterval::ALL {
            let label = IntervalLabel::of(interval);
            assert_eq!(label.parse(), Some(interval));
        }
        assert_eq!(IntervalLabel("45s".to_string()).parse(), None);
        assert_eq!(
            IntervalLabel("1M".to_string()).parse(),
            Some(ChartInterval::Month1)
        );
        assert_eq!(
            IntervalLabel("1m".to_string()).parse(),
            Some(ChartInterval::Minute1)
        );
    }
}
