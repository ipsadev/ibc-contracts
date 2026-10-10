// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::time::Instant;

use alloy_sol_types::SolValue;
use ipsa_scp_verifier::steps::VerifiedLedger;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::ProveRequest;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use sp1_sdk::HashableKey;
use sp1_sdk::ProvingKey;
use sp1_sdk::SP1ProofMode;
use sp1_sdk::SP1ProofWithPublicValues;
use sp1_sdk::SP1VerifyingKey;
use sp1_stellar_prover::Fixture;
use stellar_light_client_update_client::ConsensusState;
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

    let mode = match std::env::var("SP1_PROOF_MODE").as_deref() {
        Ok("compressed") => SP1ProofMode::Compressed,
        Ok("groth16") => SP1ProofMode::Groth16,
        Ok("plonk") => SP1ProofMode::Plonk,
        _ => SP1ProofMode::Core,
    };

    let network = matches!(std::env::var("SP1_PROVER").as_deref(), Ok("network"));

    if network && std::env::var("NETWORK_PRIVATE_KEY").is_err() {
        panic!(
            "SP1_PROVER=network needs NETWORK_PRIVATE_KEY, the requester account whose deposited PROVE pays for the proof"
        );
    }

    let fixture = Fixture::load(&fixture_path);

    let unbound = fixture.verify_natively(None);
    let native = fixture.verify_natively(Some(unbound.previous_ledger_hash));
    let witness = match std::env::var("STELLAR_TRUSTED_CONSENSUS") {
        Ok(path) => {
            let (trusted, trusted_height) = trusted_consensus_state(&PathBuf::from(&path));

            println!("trusted  {path} at ledger {trusted_height}");

            fixture.witness_at(VERIFICATION_TIME, trusted_height, trusted)
        }
        Err(_) => fixture.witness(&native, VERIFICATION_TIME),
    };

    let elf_bytes = std::fs::read(&elf_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", elf_path.display()));

    println!("fixture  {}", fixture_path.display());
    println!("slot     {}", fixture.ledger_seq);
    println!("mode     {mode:?}");
    println!(
        "prover   {}",
        if network {
            "succinct network"
        } else {
            "local cpu"
        }
    );
    println!();

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&witness.client_state.abi_encode());
    stdin.write_slice(&witness.trusted_consensus_state.abi_encode());
    stdin.write_slice(&witness.proposed_header.abi_encode());
    stdin.write_slice(&witness.time.to_le_bytes());

    if network {
        let client = ProverClient::builder().network().build();

        prove_and_report(&client, elf_bytes, stdin, mode, &fixture, &native);
    } else {
        let client = ProverClient::builder().cpu().build();

        prove_and_report(&client, elf_bytes, stdin, mode, &fixture, &native);
    }
}

fn prove_and_report<P>(
    client: &P,
    elf_bytes: Vec<u8>,
    stdin: SP1Stdin,
    mode: SP1ProofMode,
    fixture: &Fixture,
    native: &VerifiedLedger,
) where
    P: Prover,
{
    let started = Instant::now();

    let proving_key = client
        .setup(Elf::from(elf_bytes))
        .expect("the guest ELF could not be set up");

    let verifying_key = proving_key.verifying_key().clone();

    println!("setup    {:.1}s", started.elapsed().as_secs_f64());
    println!("vkey     {}", verifying_key.bytes32());
    println!();

    let started = Instant::now();

    let proof = client
        .prove(&proving_key, stdin)
        .mode(mode)
        .run()
        .expect("proving failed");

    let proving_seconds = started.elapsed().as_secs_f64();

    println!("proved   {proving_seconds:.1}s");

    client
        .verify(&proof, &verifying_key, None)
        .expect("the proof did not verify");

    println!("verified against the verifying key");

    export(&proof, &verifying_key, mode, fixture, proving_seconds);

    let output = UpdateClientOutput::abi_decode(proof.public_values.as_slice())
        .expect("the committed public values are not an UpdateClientOutput");

    assert_eq!(output.newConsensusState.ledgerHash.0, native.ledger_hash);
    assert_eq!(
        output.newConsensusState.bucketListHash.0,
        native.bucket_list_hash
    );
    assert_eq!(output.newHeight.revisionHeight, native.slot);

    println!();
    println!("slot            {}", output.newHeight.revisionHeight);
    println!(
        "ledger hash     {}",
        hex::encode(output.newConsensusState.ledgerHash.0)
    );
    println!(
        "bucket list     {}",
        hex::encode(output.newConsensusState.bucketListHash.0)
    );
    println!(
        "trust root      {}",
        hex::encode(output.clientState.trustRootFingerprint.0)
    );
}

fn export(
    proof: &SP1ProofWithPublicValues,
    verifying_key: &SP1VerifyingKey,
    mode: SP1ProofMode,
    fixture: &Fixture,
    proving_seconds: f64,
) {
    let Ok(destination) = std::env::var("SP1_PROOF_EXPORT") else {
        return;
    };

    let path = PathBuf::from(&destination);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("cannot create {}: {error}", parent.display()));
    }

    let bytes = proof.bytes();

    let json = format!(
        r#"{{
  "mode": "{mode:?}",
  "vkey": "{vkey}",
  "publicValues": "0x{public_values}",
  "proof": "0x{proof_bytes}",
  "slot": {slot},
  "provingSeconds": {proving_seconds:.1}
}}
"#,
        vkey = verifying_key.bytes32(),
        public_values = hex::encode(proof.public_values.as_slice()),
        proof_bytes = hex::encode(&bytes),
        slot = fixture.ledger_seq,
    );

    std::fs::write(&path, json)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));

    println!("exported {} ({} proof bytes)", path.display(), bytes.len());
}

fn trusted_consensus_state(path: &PathBuf) -> (ConsensusState, u64) {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    let json: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));

    let field = |name: &str| -> String {
        json.get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{} carries no {name}", path.display()))
            .trim_start_matches("0x")
            .to_string()
    };

    let close_time = json
        .get("closeTime")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("{} carries no closeTime", path.display()));

    let height = json
        .get("revisionHeight")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("{} carries no revisionHeight", path.display()));

    let state = ConsensusState {
        closeTime: close_time,
        ledgerHash: sp1_stellar_prover::decode_hex32(&field("ledgerHash")).into(),
        previousLedgerHash: sp1_stellar_prover::decode_hex32(&field("previousLedgerHash")).into(),
        bucketListHash: sp1_stellar_prover::decode_hex32(&field("bucketListHash")).into(),
        transactionSetResultHash: sp1_stellar_prover::decode_hex32(&field(
            "transactionSetResultHash",
        ))
        .into(),
    };

    (state, height)
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
