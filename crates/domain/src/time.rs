//! Pure time arithmetic on Unix nanoseconds. Reading a clock is I/O and
//! belongs to a runtime, never to the domain.

/// Nanoseconds in one second.
pub const NANOS_PER_SECOND: i64 = 1_000_000_000;
/// Nanoseconds in one millisecond.
pub const NANOS_PER_MILLISECOND: i64 = 1_000_000;

/// Whole seconds at or before a nanosecond instant, floor division so
/// pre-epoch instants bucket consistently.
#[must_use]
pub const fn seconds_floor(unix_nanos: i64) -> i64 {
    unix_nanos.div_euclid(NANOS_PER_SECOND)
}

/// Start of the fixed-cadence bucket containing a nanosecond instant.
#[must_use]
pub fn bucket_start_unix_nanos(unix_nanos: i64, interval_seconds: u32) -> i64 {
    let interval = i64::from(interval_seconds).max(1);
    unix_nanos.div_euclid(interval * NANOS_PER_SECOND) * interval * NANOS_PER_SECOND
}

/// Converts whole milliseconds to nanoseconds, saturating instead of wrapping.
#[must_use]
pub const fn millis_to_nanos(unix_millis: i64) -> i64 {
    unix_millis.saturating_mul(NANOS_PER_MILLISECOND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_floor_to_the_cadence_boundary() {
        assert_eq!(bucket_start_unix_nanos(61_000_000_000, 60), 60_000_000_000);
        assert_eq!(bucket_start_unix_nanos(59_999_999_999, 60), 0);
        assert_eq!(bucket_start_unix_nanos(-1, 60), -60_000_000_000);
        assert_eq!(seconds_floor(-1), -1);
        assert_eq!(
            millis_to_nanos(1_700_000_000_000),
            1_700_000_000_000_000_000
        );
        assert_eq!(millis_to_nanos(i64::MAX), i64::MAX);
    }
}
