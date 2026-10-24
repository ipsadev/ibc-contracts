// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use alloy_sol_types::SolValue;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use sp1_stellar_prover::decode_hex32;
use sp1_stellar_prover::Fixture;
use stellar_light_client_update_client::UpdateClientOutput;

const ELF_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-update-client";
const FIXTURE_PATH: &str = "packages/stellar-light-client/fixtures/mainnet-63907880.json";
const VERIFICATION_TIME: u128 = 1_700_000_000_000_000_000;

fn main() {
    let root = repository_root();

    let fixture_path = match std::env::args().nth(1) {
        Some(given) => PathBuf::from(given),
        None => root.join(FIXTURE_PATH),
    };

    let elf_path = match std::env::var("SP1_STELLAR_ELF") {
        Ok(given) => PathBuf::from(given),
        Err(_) => root.join(ELF_PATH),
    };

    let fixture = Fixture::load(&fixture_path);

    println!("fixture   {}", fixture_path.display());
    println!("network   {}", fixture.network);
    println!("slot      {}", fixture.ledger_seq);
    println!("envelopes {}", fixture.envelopes_xdr.len());
    println!();

    let unbound = fixture.verify_natively(None);
    let native = fixture.verify_natively(Some(unbound.previous_ledger_hash));
    let witness = fixture.witness(&native, VERIFICATION_TIME);

    let elf_bytes = std::fs::read(&elf_path).unwrap_or_else(|error| {
        panic!(
            "cannot read the guest ELF at {}: {error}\nbuild it with: cd ibc-solidity/programs/sp1-stellar-programs/update-client && cargo prove build --locked",
            elf_path.display()
        )
    });

    println!(
        "guest ELF {} ({} bytes)",
        elf_path.display(),
        elf_bytes.len()
    );

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&witness.client_state.abi_encode());
    stdin.write_slice(&witness.trusted_consensus_state.abi_encode());
    stdin.write_slice(&witness.proposed_header.abi_encode());
    stdin.write_slice(&witness.time.to_le_bytes());

    let client = ProverClient::builder().cpu().build();

    let (public_values, report) = client
        .execute(Elf::from(elf_bytes), stdin)
        .run()
        .expect("the guest program failed to execute");

    let cycles = report.total_instruction_count();

    println!("cycles    {cycles}");
    println!();

    let output = UpdateClientOutput::abi_decode(public_values.as_slice())
        .expect("the committed public values are not an UpdateClientOutput");

    let mut failures = Vec::new();

    compare(
        &mut failures,
        "close time",
        output.newConsensusState.closeTime,
        native.close_time,
    );
    compare(
        &mut failures,
        "ledger hash",
        output.newConsensusState.ledgerHash.0,
        native.ledger_hash,
    );
    compare(
        &mut failures,
        "previous ledger hash",
        output.newConsensusState.previousLedgerHash.0,
        native.previous_ledger_hash,
    );
    compare(
        &mut failures,
        "bucket list hash",
        output.newConsensusState.bucketListHash.0,
        native.bucket_list_hash,
    );
    compare(
        &mut failures,
        "transaction set result hash",
        output.newConsensusState.transactionSetResultHash.0,
        native.tx_set_result_hash,
    );
    compare(
        &mut failures,
        "new height",
        output.newHeight.revisionHeight,
        native.slot,
    );
    compare(
        &mut failures,
        "trusted height",
        output.trustedHeight.revisionHeight,
        native.slot - 1,
    );
    compare(
        &mut failures,
        "network id echo",
        output.clientState.networkId.0,
        witness.client_state.networkId.0,
    );
    compare(
        &mut failures,
        "trust root fingerprint echo",
        output.clientState.trustRootFingerprint.0,
        witness.client_state.trustRootFingerprint.0,
    );
    compare(&mut failures, "time echo", output.time, VERIFICATION_TIME);

    compare(
        &mut failures,
        "ledger hash against the archive",
        output.newConsensusState.ledgerHash.0,
        decode_hex32(&fixture.ledger_hash),
    );
    compare(
        &mut failures,
        "close time against the archive",
        output.newConsensusState.closeTime,
        fixture.close_time,
    );
    compare(
        &mut failures,
        "transaction set result hash against the archive",
        output.newConsensusState.transactionSetResultHash.0,
        decode_hex32(&fixture.tx_set_result_hash),
    );

    if !fixture.bucket_list_hash.is_empty() {
        compare(
            &mut failures,
            "bucket list hash against the archive",
            output.newConsensusState.bucketListHash.0,
            decode_hex32(&fixture.bucket_list_hash),
        );
    }

    println!();

    if failures.is_empty() {
        println!("the circuit and the native verifier agree on every field");
        return;
    }

    for failure in &failures {
        println!("MISMATCH {failure}");
    }

    std::process::exit(1);
}

fn compare<T: PartialEq + std::fmt::Debug>(
    failures: &mut Vec<String>,
    what: &str,
    circuit: T,
    native: T,
) {
    if circuit == native {
        println!("  ok   {what}");
        return;
    }

    failures.push(format!("{what}: circuit {circuit:?}, native {native:?}"));
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
