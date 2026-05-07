//! Membership and non-membership proof verification.
//!
//! This module provides functions for verifying IBC commitment proofs
//! using Merkle-Patricia Trie proofs against the Ethereum state.

use crate::client_state::ClientState;
use crate::commitment::{
    calculate_ibc_commitment_storage_location, decode_eip1184_rlp_proof, keccak_256,
};
use crate::consensus_state::ConsensusState;
use crate::errors::Error;
use alloc::string::String;
use alloc::vec::Vec;
use ethereum_consensus::types::H256;
use ethereum_light_client_verifier::execution::ExecutionVerifier;
use light_client::types::{ClientId, Height};

/// Verifies that a value exists at the given path in the IBC contract storage.
///
/// This function verifies a membership proof by:
/// 1. Validating the proof height against the client state
/// 2. Computing the storage key from the IBC path
/// 3. Verifying the value's keccak256 hash exists at that key
///
/// # Returns
///
/// Returns the keccak256 hash of the value on success.
///
/// # Errors
///
/// - [`Error::UnexpectedProofHeight`]: Proof height exceeds client's latest height
/// - [`Error::UnexpectedStorageRoot`]: Storage root is zero
/// - [`Error::MembershipVerification`]: Proof verification failed
pub fn verify_membership<CS: ClientState, CSS: ConsensusState>(
    client_state: &CS,
    consensus_state: &CSS,
    client_id: ClientId,
    path: String,
    value: Vec<u8>,
    proof_height: Height,
    proof: Vec<u8>,
    execution_verifier: &ExecutionVerifier,
) -> Result<[u8; 32], Error> {
    let ValidateMembershipResult {
        storage_proof,
        storage_key,
        storage_root,
    } = validate_membership_args::<CS, CSS>(
        client_state,
        consensus_state,
        &client_id,
        &path,
        &proof_height,
        proof,
    )?;

    let value_hash = keccak_256(&value);

    execution_verifier
        .verify_membership(
            storage_root,
            storage_key.as_bytes(),
            rlp::encode(&trim_left_zero(&value_hash)).as_ref(),
            storage_proof,
        )
        .map_err(|e| Error::MembershipVerification {
            path,
            root: storage_root,
            value,
            error: e,
        })?;
    Ok(value_hash)
}

/// Verifies that no value exists at the given path in the IBC contract storage.
///
/// This function verifies a non-membership proof by:
/// 1. Validating the proof height against the client state
/// 2. Computing the storage key from the IBC path
/// 3. Verifying no value exists at that key
///
/// # Errors
///
/// - [`Error::UnexpectedProofHeight`]: Proof height exceeds client's latest height
/// - [`Error::UnexpectedStorageRoot`]: Storage root is zero
/// - [`Error::NonMembershipVerification`]: Proof verification failed
pub fn verify_non_membership<CS: ClientState, CSS: ConsensusState>(
    client_state: &CS,
    consensus_state: &CSS,
    client_id: ClientId,
    path: String,
    proof_height: Height,
    proof: Vec<u8>,
    execution_verifier: &ExecutionVerifier,
) -> Result<(), Error> {
    let ValidateMembershipResult {
        storage_proof,
        storage_key,
        storage_root,
    } = validate_membership_args::<CS, CSS>(
        client_state,
        consensus_state,
        &client_id,
        &path,
        &proof_height,
        proof,
    )?;

    execution_verifier
        .verify_non_membership(storage_root, storage_key.as_bytes(), storage_proof)
        .map_err(|e| Error::NonMembershipVerification {
            path,
            root: storage_root,
            error: e,
        })?;
    Ok(())
}

struct ValidateMembershipResult {
    storage_proof: Vec<Vec<u8>>,
    storage_key: H256,
    storage_root: H256,
}

fn validate_membership_args<CS: ClientState, CSS: ConsensusState>(
    client_state: &CS,
    consensus_state: &CSS,
    _client_id: &ClientId,
    path: &str,
    proof_height: &Height,
    proof: Vec<u8>,
) -> Result<ValidateMembershipResult, Error> {
    let proof_height = *proof_height;
    if client_state.latest_height() < proof_height {
        return Err(Error::UnexpectedProofHeight {
            proof_height,
            latest_height: client_state.latest_height(),
        });
    }
    let root = consensus_state.storage_root();
    let proof = decode_eip1184_rlp_proof(proof)?;
    if root.is_zero() {
        return Err(Error::UnexpectedStorageRoot {
            proof_height,
            latest_height: client_state.latest_height(),
        });
    }
    let key = calculate_ibc_commitment_storage_location(&client_state.ibc_commitments_slot(), path);

    Ok(ValidateMembershipResult {
        storage_proof: proof,
        storage_key: key,
        storage_root: root,
    })
}

fn trim_left_zero(value: &[u8]) -> &[u8] {
    let mut pos = 0;
    for v in value {
        if *v != 0 {
            break;
        }
        pos += 1;
    }
    &value[pos..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use hex_literal::hex;
    use light_client::types::Any;

    // Mock ClientState implementation for testing
    struct MockClientState {
        latest_height: Height,
        ibc_commitments_slot: H256,
        is_frozen: bool,
    }

    impl ClientState for MockClientState {
        fn is_frozen(&self) -> bool {
            self.is_frozen
        }

        fn latest_height(&self) -> Height {
            self.latest_height
        }

        fn ibc_commitments_slot(&self) -> H256 {
            self.ibc_commitments_slot
        }

        fn canonicalize(self) -> Self {
            self
        }
    }

    // Mock ConsensusState implementation for testing
    struct MockConsensusState {
        storage_root: H256,
    }

    impl TryFrom<Any> for MockConsensusState {
        type Error = ();
        fn try_from(_: Any) -> Result<Self, Self::Error> {
            Err(())
        }
    }

    impl TryInto<Any> for MockConsensusState {
        type Error = ();
        fn try_into(self) -> Result<Any, Self::Error> {
            Err(())
        }
    }

    impl crate::consensus_state::ConsensusState for MockConsensusState {
        fn storage_root(&self) -> H256 {
            self.storage_root
        }
    }

    // Helper functions to reduce test code duplication
    fn create_mock_client_state_with_slot(
        latest_height: u64,
        ibc_commitments_slot: H256,
    ) -> MockClientState {
        MockClientState {
            latest_height: Height::new(0, latest_height),
            ibc_commitments_slot,
            is_frozen: false,
        }
    }

    fn create_mock_client_state(latest_height: u64) -> MockClientState {
        create_mock_client_state_with_slot(latest_height, H256::from_slice(&[1u8; 32]))
    }

    fn create_mock_consensus_state(storage_root: Option<H256>) -> MockConsensusState {
        MockConsensusState {
            storage_root: storage_root.unwrap_or_else(|| H256::from_slice(&[2u8; 32])),
        }
    }

    fn create_test_rlp_proof() -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(1);
        stream.begin_list(2);
        stream.append(&vec![1u8, 2, 3]);
        stream.append(&vec![4u8, 5, 6]);
        stream.out().to_vec()
    }

    #[test]
    fn test_trim_left_zero_no_zeros() {
        let input = [1u8, 2, 3, 4];
        let result = trim_left_zero(&input);
        assert_eq!(result, &[1u8, 2, 3, 4]);
    }

    #[test]
    fn test_trim_left_zero_with_leading_zeros() {
        let input = [0u8, 0, 0, 1, 2, 3];
        let result = trim_left_zero(&input);
        assert_eq!(result, &[1u8, 2, 3]);
    }

    #[test]
    fn test_trim_left_zero_all_zeros() {
        let input = [0u8, 0, 0, 0];
        let result = trim_left_zero(&input);
        assert_eq!(result, &[] as &[u8]);
    }

    #[test]
    fn test_trim_left_zero_empty() {
        let input: [u8; 0] = [];
        let result = trim_left_zero(&input);
        assert_eq!(result, &[] as &[u8]);
    }

    #[test]
    fn test_trim_left_zero_single_nonzero() {
        let input = [5u8];
        let result = trim_left_zero(&input);
        assert_eq!(result, &[5u8]);
    }

    #[test]
    fn test_validate_membership_args_proof_height_too_high() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let proof_height = Height::new(0, 200); // Higher than latest_height

        let result = validate_membership_args::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            &client_id,
            "test/path",
            &proof_height,
            create_test_rlp_proof(),
        );

        assert!(result.is_err());
        match result {
            Err(Error::UnexpectedProofHeight {
                proof_height: ph,
                latest_height: lh,
            }) => {
                assert_eq!(ph, Height::new(0, 200));
                assert_eq!(lh, Height::new(0, 100));
            }
            _ => panic!("Expected UnexpectedProofHeight error"),
        }
    }

    #[test]
    fn test_validate_membership_args_storage_root_zero() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(Some(H256::default()));
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let proof_height = Height::new(0, 50);

        let result = validate_membership_args::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            &client_id,
            "test/path",
            &proof_height,
            create_test_rlp_proof(),
        );

        assert!(result.is_err());
        match result {
            Err(Error::UnexpectedStorageRoot { .. }) => {}
            _ => panic!("Expected UnexpectedStorageRoot error"),
        }
    }

    #[test]
    fn test_validate_membership_args_invalid_proof_format() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let proof_height = Height::new(0, 50);

        let result = validate_membership_args::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            &client_id,
            "test/path",
            &proof_height,
            vec![0x80], // Invalid: RLP encoding of empty string
        );

        assert!(result.is_err());
        match result {
            Err(Error::InvalidProofFormat { .. }) => {}
            _ => panic!("Expected InvalidProofFormat error"),
        }
    }

    #[test]
    fn test_validate_membership_args_success() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let path = "clients/07-tendermint-0/clientState";
        let proof_height = Height::new(0, 50);

        let result = validate_membership_args::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            &client_id,
            path,
            &proof_height,
            create_test_rlp_proof(),
        );

        assert!(result.is_ok());
        let ValidateMembershipResult {
            storage_proof,
            storage_key,
            storage_root,
        } = result.unwrap();

        assert_eq!(storage_root, H256::from_slice(&[2u8; 32]));
        assert!(!storage_proof.is_empty());
        let expected_key =
            calculate_ibc_commitment_storage_location(&H256::from_slice(&[1u8; 32]), path);
        assert_eq!(storage_key, expected_key);
    }

    #[test]
    fn test_verify_membership_invalid_proof() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let path = "test/path".to_string();

        let result = verify_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            path.clone(),
            vec![1, 2, 3, 4],
            Height::new(0, 50),
            create_test_rlp_proof(),
            &ExecutionVerifier,
        );

        assert!(result.is_err());
        match result {
            Err(Error::MembershipVerification { path: p, .. }) => {
                assert_eq!(p, path);
            }
            _ => panic!("Expected MembershipVerification error"),
        }
    }

    #[test]
    fn test_verify_non_membership_invalid_proof() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();
        let path = "test/path".to_string();

        let result = verify_non_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            path.clone(),
            Height::new(0, 50),
            create_test_rlp_proof(),
            &ExecutionVerifier,
        );

        assert!(result.is_err());
        match result {
            Err(Error::NonMembershipVerification { path: p, .. }) => {
                assert_eq!(p, path);
            }
            _ => panic!("Expected NonMembershipVerification error"),
        }
    }

    #[test]
    fn test_verify_membership_proof_height_error() {
        let client_state = create_mock_client_state(50);
        let consensus_state = create_mock_consensus_state(None);
        let client_id = ClientId::new("07-tendermint", 0).unwrap();

        let result = verify_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            "test/path".to_string(),
            vec![1, 2, 3, 4],
            Height::new(0, 100), // Higher than latest_height
            create_test_rlp_proof(),
            &ExecutionVerifier,
        );

        assert!(result.is_err());
        match result {
            Err(Error::UnexpectedProofHeight { .. }) => {}
            _ => panic!("Expected UnexpectedProofHeight error"),
        }
    }

    // Test fixtures from ethereum-ibc-rs
    fn get_membership_test_fixtures() -> (H256, H256, String, Vec<u8>, Vec<u8>) {
        let ibc_commitments_slot = H256::from_slice(&hex!(
            "1ee222554989dda120e26ecacf756fe1235cd8d726706b57517715dde4f0c900"
        ));
        let storage_root = H256::from_slice(&hex!(
            "27cd08827e6bf1e435832f4b2660107beb562314287b3fa534f3b189574c0cca"
        ));
        let path = "clients/lcp-client-0/clientState".to_string();
        let proof = hex!("f90159f901118080a0143145e818eeff83817419a6632ea193fd1acaa4f791eb17282f623f38117f56a0e6ee0a993a7254ee9253d766ea005aec74eb1e11656961f0fb11323f4f91075580808080a01efae04adc2e970b4af3517581f41ce2ba4ff60492d33696c1e2a5ab70cb55bba03bac3f5124774e41fb6efdd7219530846f9f6441045c4666d2855c6598cfca00a020d7122ffc86cb37228940b5a9441e9fd272a3450245c9130ca3ab00bc1cd6ef80a0047f255205a0f2b0e7d29d490abf02bfb62c3ed201c338bc7f0088fa9c5d77eda069fecc766fcb2df04eb3a834b1f4ba134df2be114479e251d9cc9b6ba493077b80a094c3ed6a7ef63a6a67e46cc9876b9b1882eeba3d28e6d61bb15cdfb207d077e180f843a03e077f3dfd0489e70c68282ced0126c62fcef50acdcb7f57aa4552b87b456b11a1a05dc044e92e82db28c96fd98edd502949612b06e8da6dd74664a43a5ed857b298").to_vec();
        let value = hex!("0a242f6962632e6c69676874636c69656e74732e6c63702e76312e436c69656e74537461746512ed010a208083673c69fe3f098ea79a799d9dbb99c39b4b4f17a1a79ef58bdf8ae86299951080f524220310fb012a1353575f48415244454e494e475f4e45454445442a1147524f55505f4f55545f4f465f44415445320e494e54454c2d53412d3030323139320e494e54454c2d53412d3030323839320e494e54454c2d53412d3030333334320e494e54454c2d53412d3030343737320e494e54454c2d53412d3030363134320e494e54454c2d53412d3030363135320e494e54454c2d53412d3030363137320e494e54454c2d53412d30303832383a14cb96f8d6c2d543102184d679d7829b39434e4eec48015001").to_vec();
        (ibc_commitments_slot, storage_root, path, proof, value)
    }

    fn get_non_membership_test_fixtures() -> (H256, H256, String, Vec<u8>) {
        let ibc_commitments_slot = H256::from_slice(&hex!(
            "1ee222554989dda120e26ecacf756fe1235cd8d726706b57517715dde4f0c900"
        ));
        let storage_root = H256::from_slice(&hex!(
            "27cd08827e6bf1e435832f4b2660107beb562314287b3fa534f3b189574c0cca"
        ));
        let path = "clients/lcp-client-1/clientState".to_string();
        let proof = hex!("f90114f901118080a0143145e818eeff83817419a6632ea193fd1acaa4f791eb17282f623f38117f56a0e6ee0a993a7254ee9253d766ea005aec74eb1e11656961f0fb11323f4f91075580808080a01efae04adc2e970b4af3517581f41ce2ba4ff60492d33696c1e2a5ab70cb55bba03bac3f5124774e41fb6efdd7219530846f9f6441045c4666d2855c6598cfca00a020d7122ffc86cb37228940b5a9441e9fd272a3450245c9130ca3ab00bc1cd6ef80a0047f255205a0f2b0e7d29d490abf02bfb62c3ed201c338bc7f0088fa9c5d77eda069fecc766fcb2df04eb3a834b1f4ba134df2be114479e251d9cc9b6ba493077b80a094c3ed6a7ef63a6a67e46cc9876b9b1882eeba3d28e6d61bb15cdfb207d077e180").to_vec();
        (ibc_commitments_slot, storage_root, path, proof)
    }

    #[test]
    fn test_verify_membership_success() {
        let (ibc_commitments_slot, storage_root, path, proof, value) =
            get_membership_test_fixtures();

        let client_state = create_mock_client_state_with_slot(100, ibc_commitments_slot);
        let consensus_state = create_mock_consensus_state(Some(storage_root));
        let client_id = ClientId::new("07-tendermint", 0).unwrap();

        let result = verify_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            path,
            value,
            Height::new(0, 1),
            proof,
            &ExecutionVerifier,
        );

        assert!(result.is_ok(), "verify_membership failed: {:?}", result);
    }

    #[test]
    fn test_verify_non_membership_success() {
        let (ibc_commitments_slot, storage_root, path, proof) = get_non_membership_test_fixtures();

        let client_state = create_mock_client_state_with_slot(100, ibc_commitments_slot);
        let consensus_state = create_mock_consensus_state(Some(storage_root));
        let client_id = ClientId::new("07-tendermint", 0).unwrap();

        let result = verify_non_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            path,
            Height::new(0, 1),
            proof,
            &ExecutionVerifier,
        );

        assert!(result.is_ok(), "verify_non_membership failed: {:?}", result);
    }

    #[test]
    fn test_verify_non_membership_storage_root_zero() {
        let client_state = create_mock_client_state(100);
        let consensus_state = create_mock_consensus_state(Some(H256::default()));
        let client_id = ClientId::new("07-tendermint", 0).unwrap();

        let result = verify_non_membership::<MockClientState, MockConsensusState>(
            &client_state,
            &consensus_state,
            client_id,
            "test/path".to_string(),
            Height::new(0, 50),
            create_test_rlp_proof(),
            &ExecutionVerifier,
        );

        assert!(result.is_err());
        match result {
            Err(Error::UnexpectedStorageRoot { .. }) => {}
            _ => panic!("Expected UnexpectedStorageRoot error"),
        }
    }
}
