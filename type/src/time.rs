//! Timestamp validation utilities.
//!
//! This module provides functions for validating timestamps in the context
//! of light client operations, including trusting period checks and
//! clock drift tolerance.

use crate::errors::Error;
use core::time::Duration;
use light_client::types::Time;

/// Creates a [`Time`] from a Unix timestamp in seconds.
///
/// # Errors
///
/// Returns [`Error::Time`] if the timestamp is invalid.
pub fn new_timestamp(second: u64) -> Result<Time, Error> {
    Time::from_unix_timestamp(second as i64, 0).map_err(Error::Time)
}

/// Validates that the trusted consensus state is within the trusting period.
///
/// The trusting period is the maximum time since the trusted state was created
/// during which it can still be used to verify new headers.
///
/// # Errors
///
/// - [`Error::CurrentTimeBeforeTrustedState`]: Current time is before the trusted state time
/// - [`Error::OutOfTrustingPeriod`]: Trusted state has expired
pub fn validate_state_timestamp_within_trusting_period(
    current_timestamp: Time,
    trusting_period: Duration,
    trusted_consensus_state_timestamp: Time,
) -> Result<(), Error> {
    if current_timestamp.lt(&trusted_consensus_state_timestamp) {
        return Err(Error::CurrentTimeBeforeTrustedState {
            current: current_timestamp,
            trusted: trusted_consensus_state_timestamp,
        });
    }
    let trusting_period_end =
        (trusted_consensus_state_timestamp + trusting_period).map_err(Error::Time)?;
    if !trusting_period_end.gt(&current_timestamp) {
        return Err(Error::OutOfTrustingPeriod {
            current_timestamp,
            trusting_period_end,
        });
    }
    Ok(())
}

/// Validates that the header timestamp is not too far in the future.
///
/// Allows for clock drift between the local clock and the chain's clock.
/// A header is considered from the future if its timestamp exceeds
/// `current_timestamp + clock_drift`.
///
/// # Errors
///
/// Returns [`Error::HeaderFromFuture`] if the header timestamp is too far in the future.
pub fn validate_header_timestamp_not_future(
    current_timestamp: Time,
    clock_drift: Duration,
    untrusted_header_timestamp: Time,
) -> Result<(), Error> {
    let drifted_current_timestamp = (current_timestamp + clock_drift).map_err(Error::Time)?;
    if !drifted_current_timestamp.gt(&untrusted_header_timestamp) {
        return Err(Error::HeaderFromFuture {
            current_timestamp,
            clock_drift,
            header_timestamp: untrusted_header_timestamp,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_timestamp() {
        let ts = new_timestamp(1000).unwrap();
        assert_eq!(ts.as_unix_timestamp_secs(), 1000);
    }

    #[test]
    fn test_new_timestamp_zero() {
        let ts = new_timestamp(0).unwrap();
        assert_eq!(ts.as_unix_timestamp_secs(), 0);
    }

    #[test]
    fn test_validate_within_trusting_period_success() {
        let trusted = new_timestamp(1000).unwrap();
        let current = new_timestamp(1500).unwrap();
        let trusting_period = Duration::from_secs(1000);

        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_within_trusting_period_expired() {
        let trusted = new_timestamp(1000).unwrap();
        let current = new_timestamp(3000).unwrap();
        let trusting_period = Duration::from_secs(1000);

        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_err());
        match result {
            Err(Error::OutOfTrustingPeriod { .. }) => {}
            _ => panic!("Expected OutOfTrustingPeriod error"),
        }
    }

    #[test]
    fn test_validate_within_trusting_period_current_before_trusted() {
        let trusted = new_timestamp(2000).unwrap();
        let current = new_timestamp(1000).unwrap();
        let trusting_period = Duration::from_secs(1000);

        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_err());
        match result {
            Err(Error::CurrentTimeBeforeTrustedState { .. }) => {}
            _ => panic!("Expected CurrentTimeBeforeTrustedState error"),
        }
    }

    #[test]
    fn test_validate_within_trusting_period_exact_boundary() {
        let trusted = new_timestamp(1000).unwrap();
        let current = new_timestamp(2000).unwrap();
        let trusting_period = Duration::from_secs(1000);

        // At exact boundary (trusted + trusting_period == current), should fail
        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_header_not_future_success() {
        let current = new_timestamp(2000).unwrap();
        let header = new_timestamp(1500).unwrap();
        let clock_drift = Duration::from_secs(100);

        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_header_not_future_within_drift() {
        let current = new_timestamp(2000).unwrap();
        let header = new_timestamp(2050).unwrap();
        let clock_drift = Duration::from_secs(100);

        // Header is 50s in future, but drift allows 100s
        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_header_not_future_exceeds_drift() {
        let current = new_timestamp(2000).unwrap();
        let header = new_timestamp(2200).unwrap();
        let clock_drift = Duration::from_secs(100);

        // Header is 200s in future, but drift only allows 100s
        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_err());
        match result {
            Err(Error::HeaderFromFuture { .. }) => {}
            _ => panic!("Expected HeaderFromFuture error"),
        }
    }

    #[test]
    fn test_validate_header_not_future_exact_boundary() {
        let current = new_timestamp(2000).unwrap();
        let header = new_timestamp(2100).unwrap();
        let clock_drift = Duration::from_secs(100);

        // At exact boundary (current + drift == header), should fail
        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_err());
    }
}
