// SPDX-License-Identifier: Apache-2.0

use stellar_proof_api::expand_environment;

fn lookup(name: &str) -> Option<String> {
    match name {
        "EVM_RPC_URL" => Some("https://eth-sepolia.example/v2/key".to_string()),
        "EMPTY" => Some(String::new()),
        _ => None,
    }
}

#[test]
fn a_configuration_without_placeholders_is_unchanged() {
    let text = r#"{"eth_rpc_url": "https://rpc.example", "price": "$5"}"#;

    assert_eq!(expand_environment(text, lookup).unwrap(), text);
}

#[test]
fn a_placeholder_is_replaced_by_its_variable() {
    let text = r#"{"a": "${EVM_RPC_URL}", "b": "${EVM_RPC_URL}/x", "c": "${EMPTY}"}"#;

    assert_eq!(
        expand_environment(text, lookup).unwrap(),
        r#"{"a": "https://eth-sepolia.example/v2/key", "b": "https://eth-sepolia.example/v2/key/x", "c": ""}"#
    );
}

#[test]
fn an_unset_variable_is_refused_by_name() {
    let error = expand_environment(r#"{"a": "${BEACON_API_URL}"}"#, lookup).unwrap_err();

    assert!(error.to_string().contains("BEACON_API_URL is not set"));
}

#[test]
fn an_unclosed_or_malformed_placeholder_is_refused() {
    assert!(expand_environment(r#"{"a": "${EVM_RPC_URL"}"#, lookup).is_err());
    assert!(expand_environment(r#"{"a": "${}"}"#, lookup).is_err());
    assert!(expand_environment(r#"{"a": "${evm rpc}"}"#, lookup).is_err());
}
