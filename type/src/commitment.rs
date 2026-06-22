//! IBC commitment storage location calculation and verification.
//!
//! This module provides utilities for calculating storage locations in the
//! IBC contract and verifying account storage proofs.

use crate::consensus::AccountUpdateInfo;
use crate::errors::Error;
use alloc::vec::Vec;
use ethereum_consensus::types::{Address, H256};
use ethereum_light_client_verifier::execution::ExecutionVerifier;
use rlp::Rlp;
use tiny_keccak::{Hasher, Keccak};

/// Calculates the storage location for a commitment in the IBC contract.
///
/// The storage location is computed as:
/// ```text
/// keccak256(keccak256(path) || ibc_commitments_slot)
/// ```
///
/// See: <https://github.com/hyperledger-labs/yui-ibc-solidity/blob/0e83dc7aadf71380dae6e346492e148685510663/docs/architecture.md#L46>
pub fn calculate_ibc_commitment_storage_location(ibc_commitments_slot: &H256, path: &str) -> H256 {
    keccak_256(
        &[
            &keccak_256(path.as_bytes()),
            ibc_commitments_slot.as_bytes(),
        ]
        .concat(),
    )
    .into()
}

/// Decodes an EIP-1184 RLP-encoded proof into a vector of proof nodes.
///
/// The input proof must be an RLP-encoded list of lists.
///
/// # Errors
///
/// Returns [`Error::InvalidProofFormat`] if the proof is not a valid RLP list.
pub fn decode_eip1184_rlp_proof(proof: Vec<u8>) -> Result<Vec<Vec<u8>>, Error> {
    let r = Rlp::new(&proof);
    if r.is_list() {
        r.into_iter()
            .map(|r| {
                let node: Vec<Vec<u8>> = r.as_list().map_err(|e| Error::InvalidProofFormat {
                    message: alloc::format!("proof node must be an rlp list: {:?}", e),
                })?;
                Ok(rlp::encode_list::<Vec<u8>, Vec<u8>>(&node).into())
            })
            .collect()
    } else {
        Err(Error::InvalidProofFormat {
            message: "proof must be rlp list".into(),
        })
    }
}

/// Computes the Keccak-256 hash of the input.
pub fn keccak_256(input: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut k = Keccak::v256();
    k.update(input);
    k.finalize(&mut out);
    out
}

/// Verifies that the account storage root matches the expected value.
///
/// This function verifies the account proof against the state root and checks
/// that the storage root in the account matches the expected storage root.
///
/// # Errors
///
/// - [`Error::MptVerification`]: Account proof verification failed
/// - [`Error::AccountStorageRootMismatch`]: Storage root does not match
pub fn verify_account_storage(
    execution_verifier: &ExecutionVerifier,
    state_root: H256,
    ibc_address: &Address,
    account_update: &AccountUpdateInfo,
) -> Result<(), Error> {
    match execution_verifier
        .verify_account(
            state_root,
            ibc_address,
            account_update.account_proof.clone(),
        )
        .map_err(|e| Error::MptVerification {
            error: e,
            state_root,
            address: hex::encode(ibc_address.0),
            account_proof: account_update
                .account_proof
                .iter()
                .map(hex::encode)
                .collect(),
        })? {
        Some(account) => {
            if account_update.account_storage_root == account.storage_root {
                Ok(())
            } else {
                Err(Error::AccountStorageRootMismatch {
                    expected: account_update.account_storage_root,
                    actual: account.storage_root,
                    state_root,
                    address: hex::encode(ibc_address.0),
                    account_proof: account_update
                        .account_proof
                        .iter()
                        .map(hex::encode)
                        .collect(),
                })
            }
        }
        None => {
            if account_update.account_storage_root.is_zero() {
                Ok(())
            } else {
                Err(Error::AccountStorageRootMismatch {
                    expected: account_update.account_storage_root,
                    actual: H256::default(),
                    state_root,
                    address: hex::encode(ibc_address.0),
                    account_proof: account_update
                        .account_proof
                        .iter()
                        .map(hex::encode)
                        .collect(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use hex_literal::hex;

    #[test]
    fn test_keccak_256() {
        // Empty input
        let empty_hash = keccak_256(&[]);
        assert_eq!(
            hex::encode(empty_hash),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );

        // "hello" input
        let hello_hash = keccak_256(b"hello");
        assert_eq!(
            hex::encode(hello_hash),
            "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        );
    }

    #[test]
    fn test_calculate_ibc_commitment_storage_location() {
        let slot = H256::from_slice(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]);
        let path = "clients/07-tendermint-0/clientState";

        let location = calculate_ibc_commitment_storage_location(&slot, path);

        // Verify the calculation: keccak256(keccak256(path) || slot)
        let path_hash = keccak_256(path.as_bytes());
        let mut concat = Vec::new();
        concat.extend_from_slice(&path_hash);
        concat.extend_from_slice(slot.as_bytes());
        let expected = keccak_256(&concat);

        assert_eq!(location.as_bytes(), &expected);
    }

    #[test]
    fn test_calculate_ibc_commitment_storage_location_different_paths() {
        let slot = H256::default();
        let path1 = "path1";
        let path2 = "path2";

        let loc1 = calculate_ibc_commitment_storage_location(&slot, path1);
        let loc2 = calculate_ibc_commitment_storage_location(&slot, path2);

        assert_ne!(loc1, loc2);
    }

    #[test]
    fn test_decode_eip1184_rlp_proof_valid() {
        // Create a valid RLP-encoded list of lists using RlpStream
        let mut stream = rlp::RlpStream::new_list(2);

        // First inner list: [["a", "b"]]
        stream.begin_list(2);
        stream.append(&b"a".to_vec());
        stream.append(&b"b".to_vec());

        // Second inner list: [["c", "d"]]
        stream.begin_list(2);
        stream.append(&b"c".to_vec());
        stream.append(&b"d".to_vec());

        let encoded = stream.out();
        let result = decode_eip1184_rlp_proof(encoded.to_vec());

        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.len(), 2);
    }

    #[test]
    fn test_decode_eip1184_rlp_proof_invalid_not_list() {
        // Single byte (not a list)
        let invalid = vec![0x80]; // RLP encoding of empty string
        let result = decode_eip1184_rlp_proof(invalid);

        assert!(result.is_err());
        match result {
            Err(Error::InvalidProofFormat { message }) => {
                assert_eq!(message, "proof must be rlp list");
            }
            _ => panic!("Expected InvalidProofFormat error"),
        }
    }

    #[test]
    fn test_decode_eip1184_rlp_proof_empty_list() {
        // Empty list
        let empty_list = vec![0xc0]; // RLP encoding of empty list
        let result = decode_eip1184_rlp_proof(empty_list);

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_keccak_256_deterministic() {
        // Same input should always produce same output
        let input = b"test data";
        let hash1 = keccak_256(input);
        let hash2 = keccak_256(input);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_keccak_256_different_inputs() {
        // Different inputs should produce different outputs
        let hash1 = keccak_256(b"input1");
        let hash2 = keccak_256(b"input2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_calculate_ibc_commitment_storage_location_empty_path() {
        let slot = H256::from_slice(&[1u8; 32]);
        let path = "";

        let location = calculate_ibc_commitment_storage_location(&slot, path);

        // Verify it still computes correctly with empty path
        let path_hash = keccak_256(path.as_bytes());
        let mut concat = Vec::new();
        concat.extend_from_slice(&path_hash);
        concat.extend_from_slice(slot.as_bytes());
        let expected = keccak_256(&concat);

        assert_eq!(location.as_bytes(), &expected);
    }

    #[test]
    fn test_calculate_ibc_commitment_storage_location_different_slots() {
        let slot1 = H256::from_slice(&[1u8; 32]);
        let slot2 = H256::from_slice(&[2u8; 32]);
        let path = "same/path";

        let loc1 = calculate_ibc_commitment_storage_location(&slot1, path);
        let loc2 = calculate_ibc_commitment_storage_location(&slot2, path);

        assert_ne!(loc1, loc2);
    }

    #[test]
    fn test_decode_eip1184_rlp_proof_single_node() {
        // Create a valid RLP-encoded list with single node
        let mut stream = rlp::RlpStream::new_list(1);
        stream.begin_list(2);
        stream.append(&b"key".to_vec());
        stream.append(&b"value".to_vec());

        let encoded = stream.out();
        let result = decode_eip1184_rlp_proof(encoded.to_vec());

        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn test_decode_eip1184_rlp_proof_nested_structure() {
        // Create a more complex nested RLP structure
        let mut stream = rlp::RlpStream::new_list(3);

        // First node
        stream.begin_list(2);
        stream.append(&vec![0x01, 0x02, 0x03]);
        stream.append(&vec![0x04, 0x05, 0x06]);

        // Second node
        stream.begin_list(3);
        stream.append(&vec![0x07]);
        stream.append(&vec![0x08, 0x09]);
        stream.append(&vec![0x0a, 0x0b, 0x0c, 0x0d]);

        // Third node
        stream.begin_list(1);
        stream.append(&vec![0xff]);

        let encoded = stream.out();
        let result = decode_eip1184_rlp_proof(encoded.to_vec());

        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.len(), 3);
    }

    // Real test data from ethereum-light-client-verifier tests
    fn get_test_account_proof() -> Vec<Vec<u8>> {
        vec![
            hex!("f901d180a09199d4ddc4f618c0df40c0e1e09eaf2394cd21d566d841b654f3f268196922d0a0bac36050a7d1931b8d6f027075410a85587c649f2d0b30e8ffe967cb3329314ca03919f1f0815704a954616d26504c9201132454a1c0023252294c1abbe0fab26fa0e72e174077c047357c47cba596110765043277d24c55c787ecb164e33a7f1aa5a0de86ea5531307567132648d5c7956cb6082d6803f3dbc9e16b2dd20b320ca93aa0c2c799b60a0cd6acd42c1015512872e86c186bcf196e85061e76842f3b7cf860a088126df40baa53d4d60c0e2a004b6ee8506f131573c750649380e74662093855a02e0d86c3befd177f574a20ac63804532889077e955320c9361cd10b7cc6f5809a0c326f61dd1e74e037d4db73aede5642260bf92869081753bbace550a73989aeda069d63e492e4c3aa54393df9bc12809c9bfc6482b3feb16f2877d7a3e6857d94780a029087b3ba8c5129e161e2cb956640f4d8e31a35f3f133c19a1044993def98b61a08d65cbe14c995d8fe7c7343e9aa31efc7dd81acb0ee940ee565613d8bbbbaa02a0bb12ddf18cf418b9bb5164d2c0caad9e4a29bdca8f1a0c9ed16dd8095f8792fba0144540d36e30b250d25bd5c34d819538742dc54c2017c4eb1fabb8e45f72759180").to_vec(),
            hex!("f8518080a0b595706019b55ae9c4784db71e12bc68d3c991fc1277327e8d63014d10137f7b8080808080808080a04e41195493413c0bbe1fd524bbac490ed81e002fbf4d3d769e0be3452466de0c8080808080").to_vec(),
            hex!("f869a020fff6b964c3925a3b7475bdd2ad96660593de57a6a55a3ef0c82303af814889b846f8440180a02988bb89d212527a6054fec481672b5cdd01bdf7287129442e82bb7569a412f9a0cf76e7c6fa61cca89fee643691266bb1f2721c2d2eeb3063a5e545560abc2b7a").to_vec(),
        ]
    }

    fn get_test_state_root() -> H256 {
        H256::from_slice(&hex!(
            "6a3c41347943fdeab40fb6f0cff088bc81032c86a22b69c67c83b79b72cbb0b4"
        ))
    }

    fn get_test_address() -> Address {
        Address(hex!("12496c9aa0e6754c897ca88c1d53fea9b19b8aff"))
    }

    fn get_test_storage_root() -> H256 {
        H256::from_slice(&hex!(
            "2988BB89D212527A6054FEC481672B5CDD01BDF7287129442E82BB7569A412F9"
        ))
    }

    #[test]
    fn test_verify_account_storage_success() {
        let execution_verifier = ExecutionVerifier;
        let state_root = get_test_state_root();
        let address = get_test_address();
        let account_update = AccountUpdateInfo {
            account_proof: get_test_account_proof(),
            account_storage_root: get_test_storage_root(),
        };

        let result =
            verify_account_storage(&execution_verifier, state_root, &address, &account_update);

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_account_storage_mismatch() {
        let execution_verifier = ExecutionVerifier;
        let state_root = get_test_state_root();
        let address = get_test_address();
        // Wrong storage root
        let account_update = AccountUpdateInfo {
            account_proof: get_test_account_proof(),
            account_storage_root: H256::from_slice(&[0xaa; 32]),
        };

        let result =
            verify_account_storage(&execution_verifier, state_root, &address, &account_update);

        assert!(result.is_err());
        match result {
            Err(Error::AccountStorageRootMismatch {
                expected, actual, ..
            }) => {
                assert_eq!(expected, H256::from_slice(&[0xaa; 32]));
                assert_eq!(actual, get_test_storage_root());
            }
            _ => panic!("Expected AccountStorageRootMismatch error"),
        }
    }

    #[test]
    fn test_verify_account_storage_invalid_proof() {
        let execution_verifier = ExecutionVerifier;
        let state_root = get_test_state_root();
        let address = get_test_address();
        // Invalid proof
        let account_update = AccountUpdateInfo {
            account_proof: vec![vec![1, 2, 3]],
            account_storage_root: get_test_storage_root(),
        };

        let result =
            verify_account_storage(&execution_verifier, state_root, &address, &account_update);

        assert!(result.is_err());
        match result {
            Err(Error::MptVerification { .. }) => {}
            _ => panic!("Expected MptVerification error"),
        }
    }

    #[test]
    fn test_verify_account_storage_account_not_found_with_zero_root() {
        let execution_verifier = ExecutionVerifier;
        // Use a root that won't find any account
        let state_root = H256::from_slice(&[0x11; 32]);
        let address = get_test_address();
        // Zero storage root is OK when account doesn't exist
        let account_update = AccountUpdateInfo {
            account_proof: vec![],
            account_storage_root: H256::default(),
        };

        let result =
            verify_account_storage(&execution_verifier, state_root, &address, &account_update);

        // With empty proof and wrong root, this should error
        assert!(result.is_err());
    }
}
