use std::path::Path;

use proof_api_stellar_to_eth::{
    soroban::{executed, field, packet, router_events, transaction_from_result},
    witness::attach,
};
use sp1_stellar_prover::{decode_contract_id, Fixture};

const RECEIVE_TRANSACTION: &str =
    "f2755a23d2c601f4e784a7f136fcd95dd4e1712a2c4f6f6f6421112f8f5af426";

const ROUTER: &str = "CA74B2JVYXUZ3EWOBHV57TUZWJQ34OIOWLMHRIDUXBDPKVM5LJG6WCYV";

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn recorded_siblings() -> Vec<[u8; 32]> {
    Fixture::load(&fixtures().join("testnet-4902459.json"))
        .packet
        .expect("the python witness is recorded")
        .siblings
        .iter()
        .map(|sibling| {
            hex::decode(sibling)
                .expect("hex")
                .try_into()
                .expect("32 bytes")
        })
        .collect()
}

fn load_transaction() -> proof_api_stellar_to_eth::soroban::Transaction {
    let raw = std::fs::read_to_string(fixtures().join("transaction-f2755a23.json"))
        .expect("saved response");
    let result: serde_json::Value = serde_json::from_str(&raw).expect("json");

    transaction_from_result(RECEIVE_TRANSACTION, &result).expect("the saved response decodes")
}

#[test]
fn the_rust_witness_reproduces_the_python_witness_for_a_real_acknowledgement() {
    let recorded = Fixture::load(&fixtures().join("testnet-4902459.json"));
    let expected = recorded
        .packet
        .as_ref()
        .expect("the python witness is recorded");
    let router = decode_contract_id(ROUTER).expect("router strkey");
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");
    let path = hex::decode(&expected.path).expect("hex");

    let mut fixture = Fixture::load(&fixtures().join("testnet-4902459.json"));

    fixture.packet = None;

    let value = attach(
        &mut fixture,
        &transaction,
        &executed,
        router,
        &path,
        &recorded_siblings(),
    )
    .expect("the witness verifies");
    let built = fixture.packet.as_ref().expect("attach sets the witness");

    assert_eq!(hex::encode(&value), expected.value);
    assert_eq!(built.value, expected.value);
    assert_eq!(built.result_index, expected.result_index);
    assert_eq!(built.success_preimage_xdr, expected.success_preimage_xdr);
    assert_eq!(built.ibc_root, expected.ibc_root);
    assert_eq!(built.siblings, expected.siblings);
    assert_eq!(built.router_contract_id, expected.router_contract_id);
}

#[test]
fn a_path_the_transaction_did_not_write_is_refused() {
    let router = decode_contract_id(ROUTER).expect("router strkey");
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");

    let mut fixture = Fixture::load(&fixtures().join("testnet-4902459.json"));

    fixture.packet = None;

    let error = attach(
        &mut fixture,
        &transaction,
        &executed,
        router,
        b"sepolia-0\x01\0\0\0\0\0\0\0\x09",
        &recorded_siblings(),
    )
    .expect_err("that path was never written");

    assert!(error.to_string().contains("did not write path"), "{error}");
}

#[test]
fn another_contract_is_not_mistaken_for_the_router() {
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");

    let mut fixture = Fixture::load(&fixtures().join("testnet-4902459.json"));
    let path = hex::decode(&fixture.packet.as_ref().expect("recorded").path).expect("hex");

    fixture.packet = None;

    let error = attach(
        &mut fixture,
        &transaction,
        &executed,
        [7_u8; 32],
        &path,
        &recorded_siblings(),
    )
    .expect_err("no ibc_root comes from that contract");

    assert!(error.to_string().contains("ibc_root"), "{error}");
}

#[test]
fn siblings_that_do_not_fold_to_the_published_root_are_refused() {
    let router = decode_contract_id(ROUTER).expect("router strkey");
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");

    let mut fixture = Fixture::load(&fixtures().join("testnet-4902459.json"));
    let path = hex::decode(&fixture.packet.as_ref().expect("recorded").path).expect("hex");
    let mut siblings = recorded_siblings();

    siblings[0] = [0xEE; 32];
    fixture.packet = None;

    let error = attach(
        &mut fixture,
        &transaction,
        &executed,
        router,
        &path,
        &siblings,
    )
    .expect_err("a stale sibling must not prove the packet");

    assert!(error.to_string().contains("do not fold"), "{error}");
    assert!(fixture.packet.is_none());
}

#[test]
fn a_short_sibling_list_is_refused() {
    let router = decode_contract_id(ROUTER).expect("router strkey");
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");

    let mut fixture = Fixture::load(&fixtures().join("testnet-4902459.json"));
    let path = hex::decode(&fixture.packet.as_ref().expect("recorded").path).expect("hex");

    fixture.packet = None;

    let error = attach(
        &mut fixture,
        &transaction,
        &executed,
        router,
        &path,
        &recorded_siblings()[..63],
    )
    .expect_err("63 siblings cannot reach the root");

    assert!(
        error.to_string().contains("expected 64 siblings"),
        "{error}"
    );
}

#[test]
fn the_received_packet_and_its_acknowledgement_are_read_from_the_router_events() {
    let router = decode_contract_id(ROUTER).expect("router strkey");
    let transaction = load_transaction();
    let executed = executed(&transaction.meta).expect("meta v4 decodes");
    let events = router_events(&executed, router);

    let received = events
        .iter()
        .find(|event| event.name == "recv_packet")
        .expect("the router emitted recv_packet");
    let decoded =
        packet(field(&received.data, "packet").expect("packet field")).expect("packet decodes");

    assert_eq!(decoded.sequence, 1);
    assert_eq!(decoded.sourceClient, "stellar-testnet-0");
    assert_eq!(decoded.destClient, "sepolia-0");
    assert_eq!(decoded.payloads[0].encoding, "application/x-solidity-abi");
    assert!(events.iter().any(|event| event.name == "write_ack"));
    assert!(events.iter().any(|event| event.name == "ibc_root"));
}
