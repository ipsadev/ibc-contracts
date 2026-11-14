// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::ProveRequest;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use sp1_sdk::HashableKey;
use sp1_sdk::ProvingKey;
use sp1_sdk::SP1ProofMode;

const ELF_PATH: &str = "programs/sp1-ethereum-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-membership";
const FIXTURES: &str = "packages/ethereum-fixtures";

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "off");
    }

    let root = repository_root();
    let fixtures = root.join(FIXTURES);

    let client_state_raw = std::fs::read(fixtures.join("client-state.json"))?;
    let consensus_raw = std::fs::read(fixtures.join("consensus-state-updated.json"))?;
    let proof_raw = std::fs::read(fixtures.join("membership-proof.json"))?;
    let path_raw = std::fs::read(fixtures.join("membership-path.json"))?;
    let value_raw = std::fs::read(fixtures.join("membership-value.bin"))?;

    let client_state: ClientState = serde_json::from_slice(&client_state_raw)?;
    let consensus_state: ConsensusState = serde_json::from_slice(&consensus_raw)?;
    let path: Vec<Vec<u8>> = serde_json::from_slice(&path_raw)?;

    println!("router         {}", client_state.ibc_contract_address);
    println!("slot           {}", consensus_state.slot);
    println!("state root     {}", consensus_state.state_root);
    println!("path           0x{}", hex::encode(path.concat()));
    println!(
        "value          {}",
        if value_raw.is_empty() {
            "(empty — non-membership)".to_string()
        } else {
            format!("0x{}", hex::encode(&value_raw))
        }
    );
    println!();

    let elf_bytes = std::fs::read(root.join(ELF_PATH))?;

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&client_state_raw);
    stdin.write_slice(&consensus_raw);
    stdin.write_slice(&proof_raw);
    stdin.write_slice(&path_raw);
    stdin.write_slice(&value_raw);

    // Proving is opt-in: executing is free and catches every encoding mistake, so it is what runs
    // by default and what CI would run.
    let mode = match std::env::var("SP1_PROOF_MODE").as_deref() {
        Ok("compressed") => Some(SP1ProofMode::Compressed),
        Ok("groth16") => Some(SP1ProofMode::Groth16),
        Ok("plonk") => Some(SP1ProofMode::Plonk),
        Ok("core") => Some(SP1ProofMode::Core),
        _ => None,
    };

    let Some(mode) = mode else {
        let prover = ProverClient::builder().cpu().build();

        let (public_values, report) = prover
            .execute(Elf::from(elf_bytes), stdin)
            .run()
            .map_err(|error| anyhow::anyhow!("the guest failed to execute: {error}"))?;

        println!("cycles         {}", report.total_instruction_count());
        println!();

        return report_output(public_values.as_slice(), &consensus_state);
    };

    let network = matches!(std::env::var("SP1_PROVER").as_deref(), Ok("network"));

    println!("mode           {mode:?}");
    println!(
        "prover         {}",
        if network {
            "succinct network"
        } else {
            "local cpu"
        }
    );

    let started = std::time::Instant::now();

    let (public_values, verifying_key, proof_bytes) = if network {
        let prover = ProverClient::builder().network().build();
        let key = prover.setup(Elf::from(elf_bytes))?;
        let verifying_key = key.verifying_key().clone();
        let proof = prover.prove(&key, stdin).mode(mode).run()?;

        prover.verify(&proof, &verifying_key, None)?;

        (
            proof.public_values.as_slice().to_vec(),
            verifying_key,
            proof.bytes(),
        )
    } else {
        let prover = ProverClient::builder().cpu().build();
        let key = prover.setup(Elf::from(elf_bytes))?;
        let verifying_key = key.verifying_key().clone();
        let proof = prover.prove(&key, stdin).mode(mode).run()?;

        prover.verify(&proof, &verifying_key, None)?;

        (
            proof.public_values.as_slice().to_vec(),
            verifying_key,
            proof.bytes(),
        )
    };

    println!("proved         {:.1}s", started.elapsed().as_secs_f64());
    println!("vkey           {}", verifying_key.bytes32());
    println!("verified against the verifying key");

    if let Ok(destination) = std::env::var("SP1_PROOF_EXPORT") {
        let path = PathBuf::from(&destination);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(
            &path,
            format!(
                "{{\n  \"mode\": \"{mode:?}\",\n  \"vkey\": \"{}\",\n  \"publicValues\": \"0x{}\",\n  \"proof\": \"0x{}\"\n}}\n",
                verifying_key.bytes32(),
                hex::encode(&public_values),
                hex::encode(&proof_bytes),
            ),
        )?;

        println!(
            "exported       {} ({} proof bytes)",
            path.display(),
            proof_bytes.len()
        );
    }

    println!();

    report_output(&public_values, &consensus_state)
}

fn report_output(committed: &[u8], consensus_state: &ConsensusState) -> anyhow::Result<()> {
    anyhow::ensure!(
        committed.len() == 104,
        "the guest committed {} bytes, expected 104",
        committed.len()
    );

    let mut slot = [0_u8; 8];

    slot.copy_from_slice(&committed[64..72]);

    println!(
        "  client state commitment  {}",
        hex::encode(&committed[0..32])
    );
    println!(
        "  consensus hash           {}",
        hex::encode(&committed[32..64])
    );
    println!("  slot                     {}", u64::from_be_bytes(slot));
    println!(
        "  kv commitment            {}",
        hex::encode(&committed[72..104])
    );
    println!();

    anyhow::ensure!(
        u64::from_be_bytes(slot) == consensus_state.slot,
        "the guest committed a slot the fixture does not name"
    );

    println!("the guest proved a real packet commitment in ICS26Router storage");

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
