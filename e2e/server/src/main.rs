//! e2e gRPC server.
//!
//! Receives prover-generated updates from the Go e2e client and runs the
//! verification steps of an Ethereum light client update — the equivalent of
//! ethereum-elc's `check_header_and_update_state` verification flow
//! (validate_updates / verify_account_storage / timestamp validations /
//! compute_sync_committees) — using this repository's crates.

#![allow(clippy::result_large_err)]

use ethereum_consensus::bls::PublicKey;
use ethereum_consensus::compute::compute_sync_committee_period_at_slot;
use ethereum_consensus::context::ChainContext;
use ethereum_consensus::sync_protocol::SyncCommitteePeriod;
use ethereum_consensus::types::{Address, H256, U64};
use ethereum_light_client_proto::ibc::core::client::v1::Height as ProtoHeight;
use ethereum_light_client_proto::ibc::lightclients::ethereum::v1::TrustedSyncCommittee as ProtoTrustedSyncCommittee;
use ethereum_light_client_types::commitment::verify_account_storage;
use ethereum_light_client_types::consensus::{
    convert_proto_to_consensus_update, convert_proto_to_execution_update,
    convert_proto_to_fork_parameters, AccountUpdateInfo, TrustedSyncCommittee,
};
use ethereum_light_client_types::time::{
    secs_to_nanos, validate_header_timestamp_not_future,
    validate_state_timestamp_within_trusting_period,
};
use ethereum_light_client_types::update::{
    compute_sync_committees, TrustedConsensusState, TrustedSyncCommitteeInfo,
};
use ethereum_light_client_verifier::consensus::SyncProtocolVerifier;
use ethereum_light_client_verifier::context::{Fraction, LightClientContext};
use ethereum_light_client_verifier::execution::ExecutionVerifier;
use std::time::Duration;
use tonic::{transport::Server, Request, Response, Status};

pub mod pb {
    tonic::include_proto!("e2e.v1");
}
use pb::verifier_server::{Verifier, VerifierServer};
use pb::{VerifyUpdateRequest, VerifyUpdateResponse};

/// Trusted consensus state view for the verifier: the sync committee period is
/// derived from the trusted slot, and an update is relevant only if it
/// finalizes a slot newer than the trusted one.
#[derive(Default)]
struct TrustedState {
    slot: U64,
    current_sync_committee: PublicKey,
    next_sync_committee: PublicKey,
}

impl TrustedSyncCommitteeInfo for TrustedState {
    fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod {
        compute_sync_committee_period_at_slot(ctx, self.slot)
    }

    fn current_sync_committee(&self) -> PublicKey {
        self.current_sync_committee.clone()
    }

    fn next_sync_committee(&self) -> PublicKey {
        self.next_sync_committee.clone()
    }

    fn is_relevant_update(&self, update_finalized_slot: U64) -> bool {
        update_finalized_slot > self.slot
    }
}

fn invalid(field: &str, e: impl std::fmt::Debug) -> Status {
    Status::invalid_argument(format!("{field}: {e:?}"))
}

fn to_h256(field: &str, bz: &[u8]) -> Result<H256, Status> {
    if bz.len() != 32 {
        return Err(Status::invalid_argument(format!(
            "{field}: expected 32 bytes, got {}",
            bz.len()
        )));
    }
    Ok(H256::from_slice(bz))
}

fn verify_update<const SYNC_COMMITTEE_SIZE: usize>(
    r: VerifyUpdateRequest,
) -> Result<VerifyUpdateResponse, Status> {
    // ---- convert the embedded ethereum.v1 messages ----
    let fork_parameters = convert_proto_to_fork_parameters(
        r.fork_parameters
            .ok_or_else(|| Status::invalid_argument("fork_parameters missing"))?,
    )
    .map_err(|e| invalid("fork_parameters", e))?;
    let consensus_update = convert_proto_to_consensus_update::<SYNC_COMMITTEE_SIZE>(
        r.consensus_update
            .ok_or_else(|| Status::invalid_argument("consensus_update missing"))?,
    )
    .map_err(|e| invalid("consensus_update", e))?;
    let execution_update = convert_proto_to_execution_update(
        r.execution_update
            .ok_or_else(|| Status::invalid_argument("execution_update missing"))?,
    )
    .map_err(|e| invalid("execution_update", e))?;
    let account_update = AccountUpdateInfo::try_from(
        r.account_update
            .ok_or_else(|| Status::invalid_argument("account_update missing"))?,
    )
    .map_err(|e| invalid("account_update", e))?;
    // Reuse the TrustedSyncCommittee proto conversion to build the typed
    // sync committee for TrustedConsensusState.
    let trusted_sync_committee: TrustedSyncCommittee<SYNC_COMMITTEE_SIZE> =
        ProtoTrustedSyncCommittee {
            trusted_height: Some(ProtoHeight {
                revision_number: 0,
                revision_height: r.trusted_slot,
            }),
            sync_committee: r.sync_committee,
            is_next: r.is_next,
        }
        .try_into()
        .map_err(|e| invalid("sync_committee", e))?;

    let ibc_address: Address = r
        .ibc_address
        .as_slice()
        .try_into()
        .map_err(|e| invalid("ibc_address", e))?;

    // ---- build the verification context (ethereum-elc build_context equivalent) ----
    let ctx = LightClientContext::new(
        fork_parameters,
        r.seconds_per_slot.into(),
        r.slots_per_epoch.into(),
        r.epochs_per_sync_committee_period.into(),
        r.genesis_time.into(),
        to_h256("genesis_validators_root", &r.genesis_validators_root)?,
        r.min_sync_committee_participants as usize,
        Fraction::new(r.trust_level_numerator, r.trust_level_denominator)
            .map_err(|e| invalid("trust_level", e))?,
        r.now_secs.into(),
    );

    let trusted_state = TrustedState {
        slot: r.trusted_slot.into(),
        current_sync_committee: PublicKey::try_from(r.trusted_current_sync_committee.clone())
            .map_err(|e| invalid("trusted_current_sync_committee", e))?,
        next_sync_committee: PublicKey::try_from(r.trusted_next_sync_committee.clone())
            .map_err(|e| invalid("trusted_next_sync_committee", e))?,
    };
    let trusted_consensus_state = TrustedConsensusState::new(
        trusted_state,
        trusted_sync_committee.sync_committee,
        trusted_sync_committee.is_next,
    )
    .map_err(|e| invalid("trusted_consensus_state", e))?;

    // ---- the check_header_and_update_state verification steps ----
    SyncProtocolVerifier::default()
        .validate_updates(
            &ctx,
            &trusted_consensus_state,
            &consensus_update,
            &execution_update,
        )
        .map_err(|e| Status::failed_precondition(format!("validate_updates: {e:?}")))?;

    verify_account_storage(
        &ExecutionVerifier,
        execution_update.state_root,
        &ibc_address,
        &account_update,
    )
    .map_err(|e| Status::failed_precondition(format!("verify_account_storage: {e:?}")))?;

    validate_state_timestamp_within_trusting_period(
        secs_to_nanos(r.now_secs),
        Duration::from_secs(r.trusting_period_secs),
        secs_to_nanos(r.trusted_timestamp_secs),
    )
    .map_err(|e| Status::failed_precondition(format!("trusting_period: {e:?}")))?;
    validate_header_timestamp_not_future(
        secs_to_nanos(r.now_secs),
        Duration::from_secs(r.max_clock_drift_secs),
        secs_to_nanos(r.header_timestamp_secs),
    )
    .map_err(|e| Status::failed_precondition(format!("clock_drift: {e:?}")))?;

    let finalized_slot = consensus_update.finalized_header.0.slot;
    let block_number = execution_update.block_number;
    let trusted_state = TrustedState {
        slot: r.trusted_slot.into(),
        current_sync_committee: PublicKey::try_from(r.trusted_current_sync_committee)
            .map_err(|e| invalid("trusted_current_sync_committee", e))?,
        next_sync_committee: PublicKey::try_from(r.trusted_next_sync_committee)
            .map_err(|e| invalid("trusted_next_sync_committee", e))?,
    };
    let new_sync_committee = compute_sync_committees(&ctx, &trusted_state, consensus_update)
        .map_err(|e| Status::failed_precondition(format!("compute_sync_committees: {e:?}")))?;

    Ok(VerifyUpdateResponse {
        finalized_slot: finalized_slot.into(),
        latest_execution_block_number: block_number.into(),
        current_sync_committee: new_sync_committee.current_sync_committee.to_vec(),
        next_sync_committee: new_sync_committee.next_sync_committee.to_vec(),
    })
}

#[derive(Default)]
struct VerifierService;

#[tonic::async_trait]
impl Verifier for VerifierService {
    async fn verify_update(
        &self,
        request: Request<VerifyUpdateRequest>,
    ) -> Result<Response<VerifyUpdateResponse>, Status> {
        let r = request.into_inner();
        println!(
            "verify_update: sync_committee_size={} trusted_slot={}",
            r.sync_committee_size, r.trusted_slot
        );
        let resp = match r.sync_committee_size {
            32 => verify_update::<32>(r),
            512 => verify_update::<512>(r),
            n => Err(Status::invalid_argument(format!(
                "unsupported sync_committee_size: {n}"
            ))),
        };
        match &resp {
            Ok(r) => println!(
                "verify_update: OK finalized_slot={} block_number={}",
                r.finalized_slot, r.latest_execution_block_number
            ),
            Err(e) => println!("verify_update: ERROR {e}"),
        }
        resp.map(Response::new)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("E2E_SERVER_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:50151".to_string())
        .parse()?;
    println!("e2e verifier server listening on {addr}");
    Server::builder()
        .add_service(VerifierServer::new(VerifierService))
        .serve(addr)
        .await?;
    Ok(())
}
