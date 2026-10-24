// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use ethereum_light_client::header::Header;
use ethereum_light_client::update::update_consensus_state;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;

const ELF_PATH: &str = "programs/sp1-ethereum-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-update-client";
const FIXTURES: &str = "packages/ethereum-fixtures";

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "off");
    }

    let root = repository_root();
    let fixtures = root.join(FIXTURES);

    let client_state_raw = std::fs::read(fixtures.join("client-state.json"))?;
    let consensus_raw = std::fs::read(fixtures.join("consensus-state.json"))?;
    let header_raw = std::fs::read(fixtures.join("header.json"))?;

    let timestamp: u64 = std::fs::read_to_string(fixtures.join("timestamp.txt"))?
        .trim()
        .parse()?;

    let client_state: ClientState = serde_json::from_slice(&client_state_raw)?;
    let consensus_state: ConsensusState = serde_json::from_slice(&consensus_raw)?;
    let header: Header = serde_json::from_slice(&header_raw)?;

    let participants: u64 = header
        .consensus_update
        .sync_aggregate
        .sync_committee_bits
        .iter()
        .map(|byte| u64::from(byte.count_ones()))
        .sum();

    println!("trusted slot   {}", consensus_state.slot);
    println!(
        "finalized slot {}",
        header.consensus_update.finalized_header.beacon().slot
    );
    println!(
        "attested slot  {}",
        header.consensus_update.attested_header.beacon().slot
    );
    println!(
        "signers        {participants} of {}",
        client_state.sync_committee_size
    );
    println!("timestamp      {timestamp}");
    println!();

    let elf_path = match std::env::var("SP1_ETHEREUM_ELF") {
        Ok(given) => PathBuf::from(given),
        Err(_) => root.join(ELF_PATH),
    };

    let elf_bytes = std::fs::read(&elf_path)
        .map_err(|error| anyhow::anyhow!("cannot read {}: {error}", elf_path.display()))?;

    println!(
        "guest ELF      {} ({} bytes)",
        elf_path.display(),
        elf_bytes.len()
    );

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&client_state_raw);
    stdin.write_slice(&consensus_raw);
    stdin.write_slice(&header_raw);
    stdin.write_slice(&timestamp.to_le_bytes());

    let prover = ProverClient::builder().cpu().build();

    let (public_values, report) = prover
        .execute(Elf::from(elf_bytes), stdin)
        .run()
        .map_err(|error| anyhow::anyhow!("the guest failed to execute: {error}"))?;

    println!("cycles         {}", report.total_instruction_count());
    println!();

    let committed = public_values.as_slice();

    anyhow::ensure!(
        committed.len() == 120,
        "the guest committed {} bytes, expected 120",
        committed.len()
    );

    let slot_at = |offset: usize| {
        let mut raw = [0_u8; 8];
        raw.copy_from_slice(&committed[offset..offset + 8]);
        u64::from_be_bytes(raw)
    };

    println!(
        "  client state commitment  {}",
        hex::encode(&committed[0..32])
    );
    println!(
        "  trusted consensus hash   {}",
        hex::encode(&committed[32..64])
    );
    println!(
        "  new consensus hash       {}",
        hex::encode(&committed[64..96])
    );
    println!("  trusted slot             {}", slot_at(96));
    println!("  new slot                 {}", slot_at(104));
    println!("  timestamp                {}", slot_at(112));
    println!();

    anyhow::ensure!(
        slot_at(96) == consensus_state.slot,
        "the guest committed a trusted slot the fixture does not name"
    );

    anyhow::ensure!(
        slot_at(104) == header.consensus_update.finalized_header.beacon().slot,
        "the guest committed a new slot that is not the finalized one"
    );

    let (_, updated, _) = update_consensus_state(
        consensus_state.clone(),
        client_state.clone(),
        header.clone(),
    )
    .map_err(|error| anyhow::anyhow!("the host refused the update: {error}"))?;

    let mut host_state = client_state.clone();

    host_state.latest_slot = consensus_state.slot;

    let expected =
        sp1_ethereum_prover::update_wire::output(&host_state, &consensus_state, &updated);

    anyhow::ensure!(
        committed == expected.as_slice(),
        "the guest committed something other than the host computed"
    );

    std::fs::write(
        fixtures.join("public-values.hex"),
        format!("0x{}\n", hex::encode(committed)),
    )?;
    std::fs::write(
        fixtures.join("consensus-state-updated.json"),
        serde_json::to_string_pretty(&updated)? + "\n",
    )?;

    println!(
        "the guest verified a real beacon header and committed exactly what the host computed"
    );

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
