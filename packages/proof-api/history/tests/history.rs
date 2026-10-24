use proof_api_history::{normalise_tx_hash, NetworkRequest, ProofEntry, ProofHistory};
use sp1_sdk::network::proto::types::ProofRequest;

fn entry(tx_hash: &str) -> ProofEntry {
    ProofEntry {
        module: "eth_to_stellar".to_string(),
        src_chain: "11155111".to_string(),
        dst_chain: "stellar-testnet".to_string(),
        kind: "recv".to_string(),
        tx_hash: tx_hash.to_string(),
        sequences: vec![1],
        step: 0,
        program: "membership".to_string(),
        prover: "network".to_string(),
        status: "fulfilled".to_string(),
        error: None,
        vkey: Some([7; 32]),
        trusted_height: Some(11_328_672),
        target_height: Some(11_341_664),
        public_values: Some(vec![1, 2, 3]),
        proof: Some(vec![4, 5, 6]),
        request: Some(NetworkRequest {
            id: "0xab".to_string(),
            cycles: Some(664_243),
            deduction_amount: Some("365100000000000000".to_string()),
            ..NetworkRequest::default()
        }),
    }
}

#[test]
fn transaction_hashes_are_stored_lowercase_without_a_prefix() {
    assert_eq!(normalise_tx_hash("0xE6B3241A"), "e6b3241a");
    assert_eq!(normalise_tx_hash("E534991C"), "e534991c");
    assert_eq!(normalise_tx_hash(" aa3db98f "), "aa3db98f");
}

#[test]
fn a_request_without_a_record_keeps_only_its_id() {
    let request = NetworkRequest::from_sdk([1; 32], None);

    assert_eq!(request.id, format!("0x{}", "01".repeat(32)));
    assert_eq!(request.cycles, None);
}

#[test]
fn the_network_record_supplies_cost_cycles_and_times() {
    let record = ProofRequest {
        tx_hash: vec![0xaa, 0xbb],
        fulfiller: Some(vec![0x12]),
        cycles: Some(624_930),
        gas_used: Some(1_163_257),
        gas_limit: 2_000_000,
        gas_price: Some(210_000_000),
        base_fee: Some("383043000000000000".to_string()),
        deduction_amount: Some("365100000000000000".to_string()),
        created_at: 1_791_652_000,
        fulfilled_at: Some(1_791_652_300),
        ..ProofRequest::default()
    };

    let request = NetworkRequest::from_sdk([2; 32], Some(&record));

    assert_eq!(request.tx_hash.as_deref(), Some("0xaabb"));
    assert_eq!(request.fulfiller.as_deref(), Some("0x12"));
    assert_eq!(request.cycles, Some(624_930));
    assert_eq!(request.gas_used, Some(1_163_257));
    assert_eq!(request.gas_limit, Some(2_000_000));
    assert_eq!(request.gas_price, Some(210_000_000));
    assert_eq!(request.base_fee.as_deref(), Some("383043000000000000"));
    assert_eq!(
        request.deduction_amount.as_deref(),
        Some("365100000000000000")
    );
    assert_eq!(request.requested_at, Some(1_791_652_000));
    assert_eq!(request.fulfilled_at, Some(1_791_652_300));
}

#[tokio::test]
async fn without_a_database_recording_is_a_no_op() {
    let history = ProofHistory::new(None);

    assert!(!history.enabled());
    history.prepare().await.unwrap();
    history.record(&entry("0xab")).await;
}

#[tokio::test]
async fn a_proof_is_written_and_read_back() {
    let Ok(url) = std::env::var("PROOF_HISTORY_TEST_DB_URL") else {
        return;
    };

    let history = ProofHistory::new(Some(url.clone()));

    history.prepare().await.unwrap();
    history.prepare().await.unwrap();
    history.record(&entry("0xE6B3241A")).await;

    let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(connection);

    let row = client
        .query_one(
            "SELECT tx_hash, sequences, program, cycles, deduction_amount, proof \
             FROM proof_requests WHERE tx_hash = 'e6b3241a' ORDER BY id DESC LIMIT 1",
            &[],
        )
        .await
        .unwrap();

    assert_eq!(row.get::<_, String>(0), "e6b3241a");
    assert_eq!(row.get::<_, Vec<i64>>(1), vec![1]);
    assert_eq!(row.get::<_, String>(2), "membership");
    assert_eq!(row.get::<_, Option<i64>>(3), Some(664_243));
    assert_eq!(
        row.get::<_, Option<String>>(4).as_deref(),
        Some("365100000000000000")
    );
    assert_eq!(row.get::<_, Option<Vec<u8>>>(5), Some(vec![4, 5, 6]));
}

fn gateway_attempt(tx_hash: &str, status: &str, error: Option<&str>) -> ProofEntry {
    ProofEntry {
        module: "forward".to_string(),
        src_chain: "stellar-testnet".to_string(),
        dst_chain: "provider".to_string(),
        kind: "relay".to_string(),
        tx_hash: tx_hash.to_string(),
        sequences: vec![17],
        step: 0,
        program: "ics23".to_string(),
        prover: "gateway".to_string(),
        status: status.to_string(),
        error: error.map(str::to_string),
        vkey: None,
        trusted_height: None,
        target_height: None,
        public_values: None,
        proof: None,
        request: None,
    }
}

#[tokio::test]
async fn retries_of_one_proof_share_a_row() {
    let Ok(url) = std::env::var("PROOF_HISTORY_TEST_DB_URL") else {
        return;
    };

    let history = ProofHistory::new(Some(url.clone()));
    let hash = format!(
        "{:064x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let hash = hash.as_str();

    history.prepare().await.unwrap();
    history
        .record(&gateway_attempt(
            hash,
            "waiting",
            Some("ledger 5354244 is not available yet"),
        ))
        .await;
    history
        .record(&gateway_attempt(
            hash,
            "failed",
            Some("header assembly failed"),
        ))
        .await;
    history
        .record(&gateway_attempt(hash, "fulfilled", None))
        .await;
    history
        .record(&gateway_attempt(hash, "fulfilled", None))
        .await;

    let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(connection);

    let rows = client
        .query(
            "SELECT status, error, attempts, last_attempt_at IS NOT NULL \
             FROM proof_requests WHERE tx_hash = $1 ORDER BY id",
            &[&hash],
        )
        .await
        .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<_, String>(0), "fulfilled");
    assert_eq!(rows[0].get::<_, Option<String>>(1), None);
    assert_eq!(rows[0].get::<_, i32>(2), 3);
    assert!(rows[0].get::<_, bool>(3));
    assert_eq!(rows[1].get::<_, String>(0), "fulfilled");
    assert_eq!(rows[1].get::<_, i32>(2), 1);
}
