// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::path::PathBuf;

use sha2::Digest;
use sha2::Sha256;
use sp1_stellar_prover::router_contract_id_from_environment;
use sp1_stellar_prover::Fixture;
use sp1_stellar_prover::ROOT_EVENT_TOPIC;
use sp1_stellar_prover::ROUTER_CONTRACT_ID_VARIABLE;

const FIXTURE_PATH: &str = "packages/stellar-light-client/fixtures/mainnet-63907880.json";
const VKEYS_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/vkeys.json";
const OUTPUT_PATH: &str = "ibc-solidity/scripts/deployments/stellar/genesis.json";
const UNSET_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

fn main() {
    let root = repository_root();

    let fixture_path = match std::env::args().nth(1) {
        Some(given) => PathBuf::from(given),
        None => root.join(FIXTURE_PATH),
    };

    let vkeys_path = root.join(VKEYS_PATH);

    let raw_vkeys = std::fs::read_to_string(&vkeys_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", vkeys_path.display()));

    let vkeys: BTreeMap<String, String> = serde_json::from_str(&raw_vkeys)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", vkeys_path.display()));

    let update_client_vkey = vkeys
        .get("update-client")
        .expect("vkeys.json carries no update-client key");

    let membership_vkey = vkeys
        .get("membership")
        .expect("vkeys.json carries no membership key");

    let uc_and_membership_vkey = vkeys
        .get("uc-and-membership")
        .expect("vkeys.json carries no uc-and-membership key");

    let router = router_contract_id_from_environment().unwrap_or_else(|| {
        eprintln!("{ROUTER_CONTRACT_ID_VARIABLE} is unset.");
        eprintln!();
        eprintln!("It is the Soroban IBC router this client will believe, and every membership");
        eprintln!("proof reads the ibc_root event from that contract and no other. A placeholder");
        eprintln!("here deploys a client that verifies update proofs correctly and can never");
        eprintln!("verify a packet, because no router on Stellar has that address.");
        eprintln!();
        eprintln!("  ROUTER_CONTRACT_ID=C... cargo run -p sp1-stellar-prover --bin genesis");
        std::process::exit(1);
    });

    let fixture = Fixture::load(&fixture_path);

    let unbound = fixture.verify_natively(None);
    let verified = fixture.verify_natively(Some(unbound.previous_ledger_hash));

    let trust_root_xdr = fixture.trust_root_xdr();
    let fingerprint: [u8; 32] = Sha256::digest(&trust_root_xdr).into();

    let output_path = root.join(OUTPUT_PATH);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("cannot create {}: {error}", parent.display()));
    }

    let json = format!(
        r#"{{
  "network": "{network}",
  "verifier": "{UNSET_ADDRESS}",
  "roleManager": "{UNSET_ADDRESS}",
  "updateClientVkey": "{update_client_vkey}",
  "membershipVkey": "{membership_vkey}",
  "ucAndMembershipVkey": "{uc_and_membership_vkey}",
  "networkId": "0x{network_id}",
  "trustRootFingerprint": "0x{trust_root}",
  "routerContractId": "0x{router}",
  "rootEventTopic": "0x{topic}",
  "revisionNumber": 0,
  "revisionHeight": {slot},
  "closeTime": {close_time},
  "ledgerHash": "0x{ledger_hash}",
  "previousLedgerHash": "0x{previous_ledger_hash}",
  "bucketListHash": "0x{bucket_list_hash}",
  "transactionSetResultHash": "0x{tx_set_result_hash}"
}}
"#,
        network = fixture.network,
        network_id = fixture.network_id,
        trust_root = hex::encode(fingerprint),
        router = hex::encode(router),
        topic = hex::encode(ROOT_EVENT_TOPIC),
        slot = verified.slot,
        close_time = verified.close_time,
        ledger_hash = hex::encode(verified.ledger_hash),
        previous_ledger_hash = hex::encode(verified.previous_ledger_hash),
        bucket_list_hash = hex::encode(verified.bucket_list_hash),
        tx_set_result_hash = hex::encode(verified.tx_set_result_hash),
    );

    std::fs::write(&output_path, &json)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", output_path.display()));

    println!("wrote {}", output_path.display());
    println!();
    println!("network        {}", fixture.network);
    println!("trusted ledger {}", verified.slot);
    println!("ledger hash    {}", hex::encode(verified.ledger_hash));
    println!("trust root     {}", hex::encode(fingerprint));
    println!("router         {}", hex::encode(router));
    println!();
    println!("This is the genesis the client is deployed with: the ledger it trusts before");
    println!("it has verified anything. Everything after it is proven against this one, so");
    println!("a wrong value here is not caught later by any proof.");
    println!();
    println!("verifier and roleManager are deliberately {UNSET_ADDRESS}.");
    println!("The verifier is the SP1 verifier gateway, which differs per chain and is not");
    println!("derivable from anything here; take it from Succinct's published deployments for");
    println!("the SP1 version these keys were built with. The roleManager is the ICS26Router");
    println!("proxy on that chain: it calls verifyMembership during recvPacket, so a client whose");
    println!("proof submitter is an account verifies headers and can never receive a packet.");
    println!("The deploy script refuses both until they are set, refuses a verifier with no");
    println!("code, and refuses a roleManager that is not an ICS26Router.");
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
