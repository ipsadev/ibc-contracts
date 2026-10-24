use proof_api_core::api::RelayByTxRequest;
use proof_api_forward::{entries, transaction_id};
use tonic::Status;

fn request(ids: Vec<Vec<u8>>) -> RelayByTxRequest {
    RelayByTxRequest {
        src_chain: "provider".to_string(),
        dst_chain: "stellar-testnet".to_string(),
        source_tx_ids: ids,
        src_packet_sequences: vec![1],
        ..RelayByTxRequest::default()
    }
}

#[test]
fn a_raw_hash_is_written_as_hex_and_text_is_kept() {
    assert_eq!(transaction_id(&[0xab; 32]), "ab".repeat(32));
    assert_eq!(transaction_id(b"E534991C3D"), "E534991C3D");
}

#[test]
fn each_forwarded_transaction_becomes_a_gateway_entry() {
    let found = entries(&request(vec![b"E534991C3D".to_vec()]), None);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].module, "forward");
    assert_eq!(found[0].prover, "gateway");
    assert_eq!(found[0].program, "ics23");
    assert_eq!(found[0].status, "fulfilled");
    assert_eq!(found[0].sequences, vec![1]);
}

#[test]
fn a_refused_forward_is_recorded_as_failed_with_the_reason() {
    let refused = Status::internal("cosmos header failed");
    let found = entries(&request(vec![b"E534991C3D".to_vec()]), Some(&refused));

    assert_eq!(found[0].status, "failed");
    assert_eq!(found[0].error.as_deref(), Some("cosmos header failed"));
}

#[test]
fn evidence_that_is_not_ready_is_recorded_as_waiting() {
    let not_ready = Status::unavailable("ledger 5354244 is not available yet");
    let found = entries(&request(vec![b"E534991C3D".to_vec()]), Some(&not_ready));

    assert_eq!(found[0].status, "waiting");
    assert_eq!(
        found[0].error.as_deref(),
        Some("ledger 5354244 is not available yet")
    );
}
