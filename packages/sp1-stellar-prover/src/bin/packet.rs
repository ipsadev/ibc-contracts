// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};
use std::time::Instant;

use alloy_sol_types::SolValue;
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
use sp1_stellar_prover::router_contract_id_from_environment;
use sp1_stellar_prover::Fixture;
use sp1_stellar_prover::ROUTER_CONTRACT_ID_VARIABLE;
use stellar_light_client_membership::membership;
use stellar_light_client_solidity_types::MembershipOutput;
use stellar_light_client_update_client::update_client;
use stellar_light_client_update_client::ConsensusState;
use stellar_light_client_update_client::UpdateClientOutput;

const FIXTURE_PATH: &str = "packages/stellar-light-client/fixtures/testnet-4854948.json";
const GENESIS_PATH: &str = "ibc-solidity/scripts/deployments/stellar/sepolia-testnet.json";
const UPDATE_ELF_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-update-client";
const MEMBERSHIP_ELF_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-membership";
const UPDATE_EXPORT: &str = "ibc-solidity/test/sp1-stellar/fixtures/packet-update-proof.json";
const MEMBERSHIP_EXPORT: &str =
    "ibc-solidity/test/sp1-stellar/fixtures/packet-membership-proof.json";
const VERIFICATION_TIME: u128 = 1_700_000_000_000_000_000;

fn main() {
    let root = repository_root();

    let router = router_contract_id_from_environment().unwrap_or_else(|| {
        eprintln!("{ROUTER_CONTRACT_ID_VARIABLE} is unset.");
        eprintln!();
        eprintln!("This harness proves a packet that a real Soroban router published, so the");
        eprintln!("router it names has to be that router. The placeholder the constructed");
        eprintln!("scenario uses would make both proofs verify against each other and against");
        eprintln!("no deployed client.");
        std::process::exit(1);
    });

    let fixture_path = path_from("STELLAR_PACKET_FIXTURE", &root, FIXTURE_PATH);
    let genesis_path = path_from("STELLAR_TRUSTED_GENESIS", &root, GENESIS_PATH);
    let update_elf_path = path_from("SP1_STELLAR_ELF", &root, UPDATE_ELF_PATH);
    let membership_elf_path = path_from("SP1_STELLAR_MEMBERSHIP_ELF", &root, MEMBERSHIP_ELF_PATH);

    let fixture = Fixture::load(&fixture_path);
    let packet = fixture.packet_witness();

    let expected_router = hex::encode(router);

    if packet.router_contract_id != expected_router {
        eprintln!(
            "the fixture's packet was published by {}",
            packet.router_contract_id
        );
        eprintln!("but {ROUTER_CONTRACT_ID_VARIABLE} names {expected_router}");
        std::process::exit(1);
    }

    let genesis = Genesis::load(&genesis_path);

    if genesis.router != expected_router {
        eprintln!("{} pins router {}", genesis_path.display(), genesis.router);
        eprintln!("but {ROUTER_CONTRACT_ID_VARIABLE} names {expected_router}");
        eprintln!();
        eprintln!("A client deployed from that config can never verify this packet. Regenerate");
        eprintln!("it: {ROUTER_CONTRACT_ID_VARIABLE}=<C...> cargo run -p sp1-stellar-prover --bin genesis");
        std::process::exit(1);
    }

    println!("fixture    {}", fixture_path.display());
    println!("genesis    {}", genesis_path.display());
    println!("router     {expected_router}");
    println!("packet     {}", packet.transaction_hash);
    println!(
        "ledger     {} from trusted {}",
        fixture.ledger_seq, genesis.height
    );
    println!("path       {}", packet.path);
    println!("value      {}", packet.value);
    println!("ibc root   {}", packet.ibc_root);
    println!();

    let witness = fixture.witness_at(VERIFICATION_TIME, genesis.height, genesis.consensus_state());

    let update = update_client(
        &witness.client_state,
        &witness.trusted_consensus_state,
        &witness.proposed_header,
        witness.time,
    )
    .expect("the update-client witness must verify natively");

    println!(
        "native update-client verified to ledger {}",
        update.newHeight.revisionHeight
    );

    let result_hash = update.newConsensusState.transactionSetResultHash.0;
    let state_root_proof = fixture.state_root_proof();
    let entries = vec![fixture.membership_entry()];

    let proven = membership(
        &update.clientState,
        result_hash,
        &state_root_proof,
        &entries,
    )
    .expect("the membership witness must verify natively");

    assert_eq!(
        hex::encode(proven.ibcRoot.0),
        packet.ibc_root,
        "the root the program derived is not the one the router published"
    );

    println!(
        "native membership derived ibc root {}",
        hex::encode(proven.ibcRoot.0)
    );
    println!("  the root came from the ledger's own result set, not from the proof's inputs");
    println!();

    let mut update_stdin = SP1Stdin::new();

    update_stdin.write_slice(&witness.client_state.abi_encode());
    update_stdin.write_slice(&witness.trusted_consensus_state.abi_encode());
    update_stdin.write_slice(&witness.proposed_header.abi_encode());
    update_stdin.write_slice(&witness.time.to_le_bytes());

    let mut membership_stdin = SP1Stdin::new();

    membership_stdin.write_slice(&update.clientState.abi_encode());
    membership_stdin.write_slice(&result_hash);
    membership_stdin.write_slice(&state_root_proof.abi_encode());
    membership_stdin.write_slice(&1_u16.to_le_bytes());
    membership_stdin.write_slice(&entries[0].abi_encode());

    let update_elf = read_elf(&update_elf_path);
    let membership_elf = read_elf(&membership_elf_path);

    let mode = match std::env::var("SP1_PROOF_MODE").as_deref() {
        Ok("compressed") => Some(SP1ProofMode::Compressed),
        Ok("groth16") => Some(SP1ProofMode::Groth16),
        Ok("plonk") => Some(SP1ProofMode::Plonk),
        Ok("core") => Some(SP1ProofMode::Core),
        _ => None,
    };

    let Some(mode) = mode else {
        let client = ProverClient::builder().cpu().build();

        execute(
            &client,
            update_elf,
            update_stdin,
            "update-client",
            |values| {
                let output = UpdateClientOutput::abi_decode(values)
                    .expect("the committed values are not an UpdateClientOutput");

                assert_eq!(
                    output.abi_encode(),
                    update.abi_encode(),
                    "the guest and the native run disagree"
                );
            },
        );

        execute(
            &client,
            membership_elf,
            membership_stdin,
            "membership",
            |values| {
                let output = MembershipOutput::abi_decode(values)
                    .expect("the committed values are not a MembershipOutput");

                assert_eq!(
                    output.abi_encode(),
                    proven.abi_encode(),
                    "the guest and the native run disagree"
                );
            },
        );

        println!();
        println!("Both guests agree with the native run over real chain data. No proof was made:");
        println!("set SP1_PROOF_MODE=groth16 and SP1_PROVER=network to produce the two proofs a");
        println!("deployed client would be given.");

        return;
    };

    let network = matches!(std::env::var("SP1_PROVER").as_deref(), Ok("network"));

    if network && std::env::var("NETWORK_PRIVATE_KEY").is_err() {
        panic!(
            "SP1_PROVER=network needs NETWORK_PRIVATE_KEY, the requester account whose deposited PROVE pays for the proof"
        );
    }

    println!("mode       {mode:?}");
    println!(
        "prover     {}",
        if network {
            "succinct network"
        } else {
            "local cpu"
        }
    );
    println!();

    if network {
        let client = ProverClient::builder().network().build();

        prove_both(
            &client,
            update_elf,
            update_stdin,
            membership_elf,
            membership_stdin,
            mode,
            &root,
            fixture.ledger_seq,
        );
    } else {
        let client = ProverClient::builder().cpu().build();

        prove_both(
            &client,
            update_elf,
            update_stdin,
            membership_elf,
            membership_stdin,
            mode,
            &root,
            fixture.ledger_seq,
        );
    }
}

fn execute<P, F>(client: &P, elf: Vec<u8>, stdin: SP1Stdin, name: &str, check: F)
where
    P: Prover,
    F: FnOnce(&[u8]),
{
    let (public_values, report) = client
        .execute(Elf::from(elf), stdin)
        .run()
        .unwrap_or_else(|error| panic!("the {name} program failed to execute: {error}"));

    check(public_values.as_slice());

    println!(
        "{name:<16} {} cycles, {} bytes committed, matches the native run",
        report.total_instruction_count(),
        public_values.as_slice().len()
    );
}

#[allow(clippy::too_many_arguments)]
fn prove_both<P>(
    client: &P,
    update_elf: Vec<u8>,
    update_stdin: SP1Stdin,
    membership_elf: Vec<u8>,
    membership_stdin: SP1Stdin,
    mode: SP1ProofMode,
    root: &Path,
    slot: u64,
) where
    P: Prover,
{
    prove_one(
        client,
        update_elf,
        update_stdin,
        mode,
        "update-client",
        &root.join(UPDATE_EXPORT),
        slot,
    );

    prove_one(
        client,
        membership_elf,
        membership_stdin,
        mode,
        "membership",
        &root.join(MEMBERSHIP_EXPORT),
        slot,
    );
}

fn prove_one<P>(
    client: &P,
    elf: Vec<u8>,
    stdin: SP1Stdin,
    mode: SP1ProofMode,
    name: &str,
    export_path: &PathBuf,
    slot: u64,
) where
    P: Prover,
{
    let proving_key = client
        .setup(Elf::from(elf))
        .unwrap_or_else(|error| panic!("the {name} ELF could not be set up: {error}"));

    let verifying_key = proving_key.verifying_key().clone();

    println!("{name:<16} vkey {}", verifying_key.bytes32());

    let started = Instant::now();

    let proof = client
        .prove(&proving_key, stdin)
        .mode(mode)
        .run()
        .unwrap_or_else(|error| panic!("proving {name} failed: {error}"));

    let seconds = started.elapsed().as_secs_f64();

    client
        .verify(&proof, &verifying_key, None)
        .unwrap_or_else(|error| panic!("the {name} proof did not verify: {error}"));

    println!("{name:<16} proved in {seconds:.1}s and verified against its verifying key");

    export(&proof, &verifying_key, mode, export_path, slot, seconds);
}

fn export(
    proof: &SP1ProofWithPublicValues,
    verifying_key: &SP1VerifyingKey,
    mode: SP1ProofMode,
    path: &PathBuf,
    slot: u64,
    seconds: f64,
) {
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
  "provingSeconds": {seconds:.1}
}}
"#,
        vkey = verifying_key.bytes32(),
        public_values = hex::encode(proof.public_values.as_slice()),
        proof_bytes = hex::encode(&bytes),
    );

    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));

    println!("{:<16} exported {}", "", path.display());
}

struct Genesis {
    router: String,
    height: u64,
    close_time: u64,
    ledger_hash: String,
    previous_ledger_hash: String,
    bucket_list_hash: String,
    transaction_set_result_hash: String,
}

impl Genesis {
    fn load(path: &PathBuf) -> Self {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

        let json: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));

        let text = |name: &str| -> String {
            json.get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{} carries no {name}", path.display()))
                .trim_start_matches("0x")
                .to_string()
        };

        let number = |name: &str| -> u64 {
            json.get(name)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| panic!("{} carries no {name}", path.display()))
        };

        Self {
            router: text("routerContractId"),
            height: number("revisionHeight"),
            close_time: number("closeTime"),
            ledger_hash: text("ledgerHash"),
            previous_ledger_hash: text("previousLedgerHash"),
            bucket_list_hash: text("bucketListHash"),
            transaction_set_result_hash: text("transactionSetResultHash"),
        }
    }

    fn consensus_state(&self) -> ConsensusState {
        ConsensusState {
            closeTime: self.close_time,
            ledgerHash: sp1_stellar_prover::decode_hex32(&self.ledger_hash).into(),
            previousLedgerHash: sp1_stellar_prover::decode_hex32(&self.previous_ledger_hash).into(),
            bucketListHash: sp1_stellar_prover::decode_hex32(&self.bucket_list_hash).into(),
            transactionSetResultHash: sp1_stellar_prover::decode_hex32(
                &self.transaction_set_result_hash,
            )
            .into(),
        }
    }
}

fn read_elf(path: &PathBuf) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}: {error}\nbuild it first: (cd {} && cargo prove build --locked)",
            path.display(),
            path.ancestors().nth(5).unwrap_or(path).display()
        )
    })
}

fn path_from(variable: &str, root: &Path, fallback: &str) -> PathBuf {
    match std::env::var(variable) {
        Ok(given) => PathBuf::from(given),
        Err(_) => root.join(fallback),
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
