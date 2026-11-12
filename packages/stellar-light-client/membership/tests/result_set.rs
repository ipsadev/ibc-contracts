// SPDX-License-Identifier: Apache-2.0

use ipsa_scp_verifier::steps::verify_result_set;

const FIXTURE: &str = "../fixtures/mainnet-63907881.json";

struct Ledger {
    tx_set_result_hash: [u8; 32],
    result_pairs: Vec<Vec<u8>>,
}

fn load() -> Ledger {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    let json: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));

    let decode = |hexed: &str| hex::decode(hexed).expect("bad hex in the fixture");

    let tx_set_result_hash: [u8; 32] = decode(json["tx_set_result_hash"].as_str().unwrap())
        .as_slice()
        .try_into()
        .expect("tx_set_result_hash is not 32 bytes");

    let result_pairs = json["result_pairs_xdr"]
        .as_array()
        .expect("the fixture carries no result pairs")
        .iter()
        .map(|one| decode(one.as_str().unwrap()))
        .collect();

    Ledger {
        tx_set_result_hash,
        result_pairs,
    }
}

#[test]
fn a_real_ledgers_results_hash_to_the_hash_its_header_committed() {
    let ledger = load();

    assert_eq!(
        ledger.result_pairs.len(),
        402,
        "mainnet 63907881 closed 402 transactions"
    );

    verify_result_set(&ledger.result_pairs, &ledger.tx_set_result_hash)
        .expect("the archived results must hash to the header's txSetResultHash");
}

#[test]
fn a_tampered_result_pair_is_rejected_on_a_real_ledger() {
    let ledger = load();

    let mut pairs = ledger.result_pairs.clone();

    *pairs[0].last_mut().unwrap() ^= 0x01;

    assert!(
        verify_result_set(&pairs, &ledger.tx_set_result_hash).is_err(),
        "one flipped byte in one of 402 results must break the set hash"
    );
}

#[test]
fn a_dropped_result_pair_is_rejected_on_a_real_ledger() {
    let ledger = load();

    let mut pairs = ledger.result_pairs.clone();

    pairs.pop();

    assert!(
        verify_result_set(&pairs, &ledger.tx_set_result_hash).is_err(),
        "the count is hashed first, so dropping a result cannot go unnoticed"
    );
}

/// The pairs are hashed in order, so a relayer that returns them sorted, or a proof API that
/// rebuilds the list from a map, produces a set that does not bind.
#[test]
fn reordered_results_are_rejected_on_a_real_ledger() {
    let ledger = load();

    let mut pairs = ledger.result_pairs.clone();

    pairs.swap(0, 1);

    assert!(
        verify_result_set(&pairs, &ledger.tx_set_result_hash).is_err(),
        "result order is part of what txSetResultHash commits to"
    );
}

/// The anchor is the consensus state's hash, not one the prover supplies. Verifying a real ledger's
/// results against another ledger's hash is what a substituted consensus state would look like.
#[test]
fn results_do_not_verify_against_another_ledgers_hash() {
    let ledger = load();

    let mut other = ledger.tx_set_result_hash;

    other[0] ^= 0x01;

    assert!(
        verify_result_set(&ledger.result_pairs, &other).is_err(),
        "results must bind to the ledger whose header committed them"
    );
}
