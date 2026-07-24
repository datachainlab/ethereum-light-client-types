//! Timestamp validation utilities.
//!
//! This module provides functions for validating timestamps in the context
//! of light client operations, including trusting period checks and
//! clock drift tolerance.
//!
//! All timestamps are Unix timestamps in nanoseconds so that this crate does
//! not depend on any specific light client framework's time type.

use crate::errors::Error;
use core::time::Duration;
use ethereum_consensus::beacon::Slot;
use ethereum_consensus::compute::compute_timestamp_at_slot;
use ethereum_consensus::context::ChainContext;

/// Validates that the trusted consensus state is still within the trusting period.
pub fn validate_state_timestamp_within_trusting_period(
    current_timestamp_nanos: u128,
    trusting_period: Duration,
    trusted_consensus_state_timestamp_nanos: u128,
) -> Result<(), Error> {
    if current_timestamp_nanos < trusted_consensus_state_timestamp_nanos {
        return Err(Error::CurrentTimeBeforeTrustedState {
            current: current_timestamp_nanos,
            trusted: trusted_consensus_state_timestamp_nanos,
        });
    }
    let trusting_period_end = trusted_consensus_state_timestamp_nanos + trusting_period.as_nanos();
    if trusting_period_end <= current_timestamp_nanos {
        return Err(Error::OutOfTrustingPeriod {
            current_timestamp: current_timestamp_nanos,
            trusting_period_end,
        });
    }
    Ok(())
}

/// Validates that the header timestamp is not in the future, allowing for `clock_drift`.
pub fn validate_header_timestamp_not_future(
    current_timestamp_nanos: u128,
    clock_drift: Duration,
    untrusted_header_timestamp_nanos: u128,
) -> Result<(), Error> {
    let drifted_current_timestamp = current_timestamp_nanos + clock_drift.as_nanos();
    if drifted_current_timestamp <= untrusted_header_timestamp_nanos {
        return Err(Error::HeaderFromFuture {
            current_timestamp: current_timestamp_nanos,
            clock_drift,
            header_timestamp: untrusted_header_timestamp_nanos,
        });
    }
    Ok(())
}

/// Validates that the header timestamp is non-zero and equals
/// `compute_timestamp_at_slot(finalized_slot)`.
pub fn validate_header_timestamp<C: ChainContext>(
    ctx: &C,
    finalized_slot: Slot,
    header_timestamp_nanos: u128,
) -> Result<(), Error> {
    if header_timestamp_nanos == 0 {
        return Err(Error::ZeroTimestamp);
    }
    let expected = secs_to_nanos(compute_timestamp_at_slot(ctx, finalized_slot).0);
    if header_timestamp_nanos != expected {
        return Err(Error::UnexpectedTimestamp {
            expected,
            actual: header_timestamp_nanos,
        });
    }
    Ok(())
}

/// Converts a Unix timestamp in seconds to nanoseconds.
pub const fn secs_to_nanos(secs: u64) -> u128 {
    secs as u128 * 1_000_000_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use ethereum_consensus::config;
    use ethereum_consensus::context::DefaultChainContext;
    use ethereum_consensus::fork::{altair::ALTAIR_FORK_SPEC, ForkParameter, ForkParameters};
    use ethereum_consensus::preset;
    use ethereum_consensus::types::U64;

    fn test_context() -> DefaultChainContext {
        // genesis_slot = 0, seconds_per_slot = 6 (minimal preset),
        // genesis_time = min_genesis_time => timestamp(slot) = 1578009600 + slot * 6
        let cfg = config::Config {
            preset: preset::minimal::PRESET,
            fork_parameters: ForkParameters::new(
                ethereum_consensus::beacon::Version([0, 0, 0, 1]),
                vec![ForkParameter::new(
                    ethereum_consensus::beacon::Version([1, 0, 0, 1]),
                    U64(0),
                    ALTAIR_FORK_SPEC,
                )],
            )
            .unwrap(),
            min_genesis_time: U64(1578009600),
        };
        DefaultChainContext::new_with_config(U64(1729846322), cfg)
    }

    #[test]
    fn test_validate_header_timestamp_success() {
        let ctx = test_context();
        let slot = Slot::from(10u64);
        let expected_secs = compute_timestamp_at_slot(&ctx, slot).0;
        let header_timestamp = secs_to_nanos(expected_secs);
        assert!(validate_header_timestamp(&ctx, slot, header_timestamp).is_ok());
    }

    #[test]
    fn test_validate_header_timestamp_zero() {
        let ctx = test_context();
        assert!(matches!(
            validate_header_timestamp(&ctx, Slot::from(10u64), 0),
            Err(Error::ZeroTimestamp)
        ));
    }

    #[test]
    fn test_validate_header_timestamp_mismatch() {
        let ctx = test_context();
        let slot = Slot::from(10u64);
        // off by one second from the slot-derived timestamp
        let expected_secs = compute_timestamp_at_slot(&ctx, slot).0;
        let header_timestamp = secs_to_nanos(expected_secs + 1);
        assert!(matches!(
            validate_header_timestamp(&ctx, slot, header_timestamp),
            Err(Error::UnexpectedTimestamp { .. })
        ));
    }

    #[test]
    fn test_secs_to_nanos() {
        assert_eq!(secs_to_nanos(1000), 1_000_000_000_000);
        assert_eq!(secs_to_nanos(0), 0);
    }

    #[test]
    fn test_validate_within_trusting_period_success() {
        let trusted = secs_to_nanos(1000);
        let current = secs_to_nanos(1500);
        let trusting_period = Duration::from_secs(1000);

        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_within_trusting_period_expired() {
        let trusted = secs_to_nanos(1000);
        let current = secs_to_nanos(3000);
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
        let trusted = secs_to_nanos(2000);
        let current = secs_to_nanos(1000);
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
        let trusted = secs_to_nanos(1000);
        let current = secs_to_nanos(2000);
        let trusting_period = Duration::from_secs(1000);

        // At exact boundary (trusted + trusting_period == current), should fail
        let result =
            validate_state_timestamp_within_trusting_period(current, trusting_period, trusted);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_header_not_future_success() {
        let current = secs_to_nanos(2000);
        let header = secs_to_nanos(1500);
        let clock_drift = Duration::from_secs(100);

        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_header_not_future_within_drift() {
        let current = secs_to_nanos(2000);
        let header = secs_to_nanos(2050);
        let clock_drift = Duration::from_secs(100);

        // Header is 50s in future, but drift allows 100s
        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_header_not_future_exceeds_drift() {
        let current = secs_to_nanos(2000);
        let header = secs_to_nanos(2200);
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
        let current = secs_to_nanos(2000);
        let header = secs_to_nanos(2100);
        let clock_drift = Duration::from_secs(100);

        // At exact boundary (current + drift == header), should fail
        let result = validate_header_timestamp_not_future(current, clock_drift, header);
        assert!(result.is_err());
    }
}
