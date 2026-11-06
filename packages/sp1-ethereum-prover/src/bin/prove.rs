// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::time::Instant;

use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::ProveRequest;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use sp1_sdk::HashableKey;
use sp1_sdk::ProvingKey;
use sp1_sdk::SP1ProofMode;

const ELF_PATH: &str = "programs/sp1-ethereum-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-update-client";
const FIXTURES: &str = "packages/ethereum-fixtures";

fn main() -> anyhow::Result<()> {
    let root = repository_root();
    let fixtures = root.join(FIXTURES);

    let client_state = std::fs::read(fixtures.join("client-state.json"))?;
    let consensus = std::fs::read(fixtures.join("consensus-state.json"))?;
    let header = std::fs::read(fixtures.join("header.json"))?;

    let timestamp: u64 = std::fs::read_to_string(fixtures.join("timestamp.txt"))?
        .trim()
        .parse()?;

    let mode = match std::env::var("SP1_PROOF_MODE").as_deref() {
        Ok("compressed") => SP1ProofMode::Compressed,
        Ok("groth16") => SP1ProofMode::Groth16,
        Ok("plonk") => SP1ProofMode::Plonk,
        _ => SP1ProofMode::Core,
    };

    let network = matches!(std::env::var("SP1_PROVER").as_deref(), Ok("network"));

    if network && std::env::var("NETWORK_PRIVATE_KEY").is_err() {
        anyhow::bail!("SP1_PROVER=network needs NETWORK_PRIVATE_KEY");
    }

    let elf_bytes = std::fs::read(root.join(ELF_PATH))?;

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

    stdin.write_slice(&client_state);
    stdin.write_slice(&consensus);
    stdin.write_slice(&header);
    stdin.write_slice(&timestamp.to_le_bytes());

    let started = Instant::now();

    let (verifying_key, proof) = if network {
        let prover = ProverClient::builder().network().build();
        let key = prover.setup(Elf::from(elf_bytes))?;
        let verifying_key = key.verifying_key().clone();

        println!("setup    {:.1}s", started.elapsed().as_secs_f64());
        println!("vkey     {}", verifying_key.bytes32());
        println!();

        let started = Instant::now();
        let proof = prover.prove(&key, stdin).mode(mode).run()?;

        println!("proved   {:.1}s", started.elapsed().as_secs_f64());

        prover.verify(&proof, &verifying_key, None)?;

        (verifying_key, proof)
    } else {
        let prover = ProverClient::builder().cpu().build();
        let key = prover.setup(Elf::from(elf_bytes))?;
        let verifying_key = key.verifying_key().clone();

        println!("setup    {:.1}s", started.elapsed().as_secs_f64());
        println!("vkey     {}", verifying_key.bytes32());
        println!();

        let started = Instant::now();
        let proof = prover.prove(&key, stdin).mode(mode).run()?;

        println!("proved   {:.1}s", started.elapsed().as_secs_f64());

        prover.verify(&proof, &verifying_key, None)?;

        (verifying_key, proof)
    };

    println!("verified against the verifying key");

    if let Ok(destination) = std::env::var("SP1_PROOF_EXPORT") {
        let path = PathBuf::from(&destination);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = proof.bytes();

        std::fs::write(
            &path,
            format!(
                "{{\n  \"mode\": \"{mode:?}\",\n  \"vkey\": \"{}\",\n  \"publicValues\": \"0x{}\",\n  \"proof\": \"0x{}\"\n}}\n",
                verifying_key.bytes32(),
                hex::encode(proof.public_values.as_slice()),
                hex::encode(&bytes),
            ),
        )?;

        println!("exported {} ({} proof bytes)", path.display(), bytes.len());
    }

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
