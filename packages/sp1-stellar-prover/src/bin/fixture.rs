// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use alloy_sol_types::SolValue;
use sp1_stellar_prover::Fixture;
use stellar_light_client_membership::membership;
use stellar_light_client_membership::scenario::client_state as membership_client_state;
use stellar_light_client_membership::scenario::entry;
use stellar_light_client_membership::scenario::proof_publishing;
use stellar_light_client_membership::scenario::Tree;
use stellar_light_client_update_client::update_client;

const FIXTURE_PATH: &str = "packages/stellar-light-client/fixtures/mainnet-63907880.json";
const OUTPUT_PATH: &str = "ibc-solidity/test/sp1-stellar/fixtures/stellar.json";
const VERIFICATION_TIME: u128 = 1_700_000_000_000_000_000;

fn main() {
    let root = repository_root();

    let fixture = Fixture::load(&root.join(FIXTURE_PATH));

    let unbound = fixture.verify_natively(None);
    let native = fixture.verify_natively(Some(unbound.previous_ledger_hash));
    let witness = fixture.witness(&native, VERIFICATION_TIME);

    let update = update_client(
        &witness.client_state,
        &witness.trusted_consensus_state,
        &witness.proposed_header,
        witness.time,
    )
    .expect("the update client witness must verify");

    let leaves: Vec<(Vec<u8>, Vec<u8>)> = (0..8_u32)
        .map(|index| {
            (
                format!("commitments/ports/transfer/channels/0/{index}").into_bytes(),
                format!("packet-{index}").into_bytes(),
            )
        })
        .collect();

    let tree = Tree::build(&leaves);

    let (state_root_proof, result_hash) = proof_publishing(tree.root);

    let membership_state = membership_client_state();

    let entries = vec![entry(&tree, &leaves[0].0, &leaves[0].1)];

    let proven = membership(&membership_state, result_hash, &state_root_proof, &entries)
        .expect("the membership witness must verify");

    let absent_key = b"receipts/ports/transfer/channels/0/999".to_vec();
    let absent = vec![entry(&tree, &absent_key, b"")];

    let proven_absent = membership(&membership_state, result_hash, &state_root_proof, &absent)
        .expect("the non-membership witness must verify");

    let document = format!(
        r#"{{
  "updateClientState": "0x{}",
  "updateTrustedConsensusState": "0x{}",
  "updateNewConsensusState": "0x{}",
  "updateTrustedHeight": {},
  "updateNewHeight": {},
  "updateClientPublicValues": "0x{}",
  "membershipClientState": "0x{}",
  "membershipConsensusState": "0x{}",
  "membershipPublicValues": "0x{}",
  "nonMembershipPublicValues": "0x{}",
  "membershipPath": "0x{}",
  "membershipValue": "0x{}",
  "nonMembershipPath": "0x{}"
}}
"#,
        hex::encode(witness.client_state.abi_encode()),
        hex::encode(witness.trusted_consensus_state.abi_encode()),
        hex::encode(update.newConsensusState.abi_encode()),
        update.trustedHeight.revisionHeight,
        update.newHeight.revisionHeight,
        hex::encode(update.abi_encode()),
        hex::encode(membership_state.abi_encode()),
        hex::encode(consensus_state_for(result_hash).abi_encode()),
        hex::encode(proven.abi_encode()),
        hex::encode(proven_absent.abi_encode()),
        hex::encode(&leaves[0].0),
        hex::encode(&leaves[0].1),
        hex::encode(&absent_key),
    );

    let output = root.join(OUTPUT_PATH);

    std::fs::create_dir_all(output.parent().expect("the output has a parent"))
        .expect("cannot create the fixture directory");

    std::fs::write(&output, document).expect("cannot write the fixture");

    println!("wrote {}", output.display());
    println!("update client slot {}", update.newHeight.revisionHeight);
    println!("membership root    {}", hex::encode(proven.ibcRoot.0));
}

fn consensus_state_for(
    result_hash: [u8; 32],
) -> stellar_light_client_solidity_types::ConsensusState {
    stellar_light_client_solidity_types::ConsensusState {
        closeTime: 1_700_000_000,
        ledgerHash: [0xaa_u8; 32].into(),
        previousLedgerHash: [0xbb_u8; 32].into(),
        bucketListHash: [0xcc_u8; 32].into(),
        transactionSetResultHash: result_hash.into(),
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
