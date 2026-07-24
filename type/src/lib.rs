//! Type definitions for Ethereum light client in the IBC ecosystem.
//!
//! This crate provides core types and utilities for implementing an Ethereum
//! light client for the IBC (Inter-Blockchain Communication) protocol.
//! It is framework-agnostic: timestamps are plain unix nanoseconds and heights
//! are a self-contained [`height::Height`], so light client frameworks such as
//! LCP can integrate via thin conversions on their side.
//!
//! # Modules
//!
//! - [`client_state`]: Trait definition for Ethereum client state
//! - [`consensus_state`]: Trait definition for Ethereum consensus state
//! - [`consensus`]: Consensus update structures and Proto conversions
//! - [`update`]: Sync committee state transition logic
//! - [`validate`]: Validation utilities for consensus and execution updates
//! - [`commitment`]: IBC commitment storage location calculation and verification
//! - [`membership`]: Membership and non-membership proof verification
//! - [`time`]: Timestamp validation utilities
//! - [`errors`]: Error types for all operations

#![allow(clippy::result_large_err)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod client_state;
pub mod commitment;
pub mod consensus;
pub mod consensus_state;
pub mod errors;
pub mod height;
pub mod membership;
pub mod time;
pub mod update;
pub mod validate;
