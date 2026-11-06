// SPDX-License-Identifier: Apache-2.0

use sp1_stellar_prover::decode_contract_id;

const ROUTER_STRKEY: &str = "CCKCPJ7LTEKCDW6QJQ35MG6E6RJOOUYI5T6DT46735ME76ZU37OOSDFQ";
const ROUTER_HEX: &str = "9427a7eb991421dbd04c37d61bc4f452e75308ecfc39f3dfdf584ffb34dfdce9";

#[test]
fn a_contract_strkey_decodes_to_the_id_stellar_reports() {
    assert_eq!(
        hex::encode(decode_contract_id(ROUTER_STRKEY).expect("the strkey is valid")),
        ROUTER_HEX
    );
}

#[test]
fn hex_is_accepted_with_and_without_a_prefix() {
    let bare = decode_contract_id(ROUTER_HEX).expect("bare hex is valid");
    let prefixed = decode_contract_id(&format!("0x{ROUTER_HEX}")).expect("prefixed hex is valid");

    assert_eq!(bare, prefixed);
    assert_eq!(hex::encode(bare), ROUTER_HEX);
}

#[test]
fn a_mistyped_strkey_is_refused_rather_than_decoded() {
    let mut mistyped = ROUTER_STRKEY.to_string();

    mistyped.replace_range(10..11, "A");

    let reason = decode_contract_id(&mistyped).expect_err("a mistyped strkey must not decode");

    assert!(
        reason.contains("checksum"),
        "the checksum is what catches it, got: {reason}"
    );
}

#[test]
fn an_account_strkey_is_refused() {
    let account = "GB2ONJBDQPX5JI7EG4XO4SIA4IMZNJ56Z3TBE2PVIOQ5FFQZEL3R3WU7";

    let reason = decode_contract_id(account).expect_err("an account is not a contract");

    assert!(
        reason.contains("not a contract"),
        "the version byte is what catches it, got: {reason}"
    );
}
