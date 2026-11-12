// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use ipsa_scp_exporter::network::Network;
use proof_api_stellar_to_eth::stellar_api::StellarApi;
use sha2::{Digest, Sha256};

const LIVE_API_URL_VARIABLE: &str = "STELLAR_LIVE_API_URL";

fn live_api_url() -> String {
    std::env::var(LIVE_API_URL_VARIABLE)
        .unwrap_or_else(|_| panic!("{LIVE_API_URL_VARIABLE} must point at a running ipsa api"))
}

fn api() -> StellarApi {
    StellarApi::new(&live_api_url(), "testnet", Duration::from_secs(60)).expect("api")
}

async fn archived_ledger() -> u32 {
    let base = Network::parse("testnet").expect("network").archive_base();

    reqwest::get(format!("{base}/.well-known/stellar-history.json"))
        .await
        .expect("archive state")
        .json::<serde_json::Value>()
        .await
        .expect("archive state json")["currentLedger"]
        .as_u64()
        .expect("current ledger") as u32
}

async fn assert_evidence_verifies(api: &StellarApi, ledger: u32) {
    let evidence = api
        .exported(ledger)
        .await
        .expect("the api's evidence verifies");
    let header = hex::decode(&evidence.ledger_header_xdr).expect("header hex");

    assert_eq!(evidence.ledger_seq, ledger as u64);
    assert_eq!(evidence.ledger_hash, hex::encode(Sha256::digest(&header)));
    assert!(!evidence.next_envelopes_xdr.is_empty());
    assert!(evidence.result_pairs_xdr.is_some());
}

#[tokio::test]
#[ignore = "needs STELLAR_LIVE_API_URL pointing at a running ipsa api"]
async fn the_api_proves_a_ledger_the_archive_has_published() {
    let api = api();
    let ledger = archived_ledger().await - 10;

    assert_evidence_verifies(&api, ledger).await;
}

#[tokio::test]
#[ignore = "needs STELLAR_LIVE_API_URL pointing at a running ipsa api"]
async fn the_api_proves_a_ledger_the_archive_has_not_published() {
    let api = api();
    let ledger = api.current_ledger().await.expect("latest ledger") - 2;

    api.wait_for(ledger)
        .await
        .expect("the api holds the ledger");

    assert_evidence_verifies(&api, ledger).await;
}

#[tokio::test]
#[ignore = "needs STELLAR_LIVE_API_URL pointing at a running ipsa api"]
async fn the_api_rebuilds_a_ledger_soroban_rpc_no_longer_keeps() {
    let api = api();

    let oldest = reqwest::Client::new()
        .post("https://soroban-testnet.stellar.org")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getHealth"
        }))
        .send()
        .await
        .expect("rpc")
        .json::<serde_json::Value>()
        .await
        .expect("rpc json")["result"]["oldestLedger"]
        .as_u64()
        .unwrap_or_else(|| panic!("no oldestLedger from the rpc"));

    assert_evidence_verifies(&api, oldest as u32 - 1_000).await;
}
