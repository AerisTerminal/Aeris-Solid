//! Clock reads, kept out of the domain.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current Unix time in nanoseconds, saturating instead of wrapping.
#[must_use]
pub fn unix_nanos_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or(i64::MAX)
}
