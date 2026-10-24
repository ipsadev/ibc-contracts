use ipsa_scp_exporter::api::{export, Options};
use ipsa_scp_exporter::network::Network;
use proof_api_stellar_to_eth::stellar_api::StellarApi;
use std::time::Duration;

#[tokio::test]
async fn the_api_assembles_the_same_evidence_as_the_archive() {
    let Ok(url) = std::env::var("STELLAR_LIVE_API_URL") else {
        return;
    };

    let api = StellarApi::new(&url, "testnet", Duration::from_secs(60)).expect("api");
    let options = Options::for_network(Network::parse("testnet").expect("network"));

    let archived = reqwest::get(format!(
        "{}/.well-known/stellar-history.json",
        options.archives[0]
    ))
    .await
    .expect("archive state")
    .json::<serde_json::Value>()
    .await
    .expect("archive state json")["currentLedger"]
        .as_u64()
        .expect("current ledger") as u32;
    let ledger = archived - 10;

    let from_api = api.exported(ledger).await.expect("the api's evidence");
    let from_archive = export(ledger, &options, None)
        .await
        .expect("the archive's evidence");

    assert_eq!(
        serde_json::to_value(&from_api).unwrap(),
        serde_json::to_value(&from_archive).unwrap(),
        "ledger {ledger}"
    );
}

#[tokio::test]
async fn the_api_proves_a_ledger_the_archive_has_not_published() {
    let Ok(url) = std::env::var("STELLAR_LIVE_API_URL") else {
        return;
    };

    let api = StellarApi::new(&url, "testnet", Duration::from_secs(60)).expect("api");
    let options = Options::for_network(Network::parse("testnet").expect("network"));
    let ledger = api.current_ledger().await.expect("latest ledger") - 2;

    let archived = reqwest::get(format!(
        "{}/.well-known/stellar-history.json",
        options.archives[0]
    ))
    .await
    .expect("archive state")
    .json::<serde_json::Value>()
    .await
    .expect("archive state json")["currentLedger"]
        .as_u64()
        .expect("current ledger") as u32;

    api.wait_for(ledger)
        .await
        .expect("the api holds the ledger");
    let evidence = api.exported(ledger).await.expect("the api's evidence");

    assert_eq!(evidence.ledger_seq, ledger as u64);
    assert!(!evidence.next_envelopes_xdr.is_empty());
    println!("ledger {ledger} assembled from the api while the archive is at {archived}");
}
