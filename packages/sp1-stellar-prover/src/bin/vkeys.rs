// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::path::PathBuf;

use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::HashableKey;
use sp1_sdk::ProvingKey;

const STELLAR_PINNED: &str = "ibc-solidity/programs/sp1-stellar-programs/vkeys.json";
const ETHEREUM_PINNED: &str = "programs/sp1-ethereum-programs/vkeys.json";

const PROGRAMS: [(&str, &str, &str); 5] = [
    (
        STELLAR_PINNED,
        "update-client",
        "ibc-solidity/programs/sp1-stellar-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-update-client",
    ),
    (
        STELLAR_PINNED,
        "membership",
        "ibc-solidity/programs/sp1-stellar-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-membership",
    ),
    (
        STELLAR_PINNED,
        "uc-and-membership",
        "ibc-solidity/programs/sp1-stellar-programs/uc-and-membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-uc-and-membership",
    ),
    (
        ETHEREUM_PINNED,
        "ethereum-update-client",
        "programs/sp1-ethereum-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-update-client",
    ),
    (
        ETHEREUM_PINNED,
        "ethereum-membership",
        "programs/sp1-ethereum-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-membership",
    ),
];

fn main() {
    let root = repository_root();

    let write = std::env::args().any(|one| one == "--write");

    let client = ProverClient::builder().cpu().build();

    let mut computed: BTreeMap<&str, BTreeMap<String, String>> = BTreeMap::new();
    let mut drifted = Vec::new();

    for (pinned_path, name, relative) in PROGRAMS {
        let pinned = read_pinned(&root.join(pinned_path), write);

        let elf_path = root.join(relative);

        let elf_bytes = std::fs::read(&elf_path).unwrap_or_else(|error| {
            panic!(
                "cannot read {}: {error}\nbuild it first: (cd {} && cargo prove build --locked)",
                elf_path.display(),
                elf_path.ancestors().nth(5).unwrap_or(&elf_path).display()
            )
        });

        let proving_key = client
            .setup(Elf::from(elf_bytes))
            .unwrap_or_else(|error| panic!("{name}: the guest ELF could not be set up: {error}"));

        let vkey = proving_key.verifying_key().bytes32();

        match pinned.get(name) {
            Some(expected) if *expected == vkey => println!("  ok    {name:<24} {vkey}"),
            Some(expected) => {
                println!("  MOVED {name:<24} {vkey}");
                println!("        {:<24} was {expected}", "");
                drifted.push(name);
            }
            None => {
                println!("  new   {name:<24} {vkey}");
                drifted.push(name);
            }
        }

        computed
            .entry(pinned_path)
            .or_default()
            .insert(name.to_string(), vkey);
    }

    println!();

    if drifted.is_empty() {
        println!("every verifying key matches what is pinned beside its programs");

        return;
    }

    if write {
        for (pinned_path, keys) in &computed {
            let mut json = serde_json::to_string_pretty(keys)
                .expect("the verifying keys could not be serialised");

            json.push('\n');

            let path = root.join(pinned_path);

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .unwrap_or_else(|error| panic!("cannot create {}: {error}", parent.display()));
            }

            std::fs::write(&path, json)
                .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));

            println!("wrote {pinned_path}");
        }

        println!();
        println!("A verifying key is a deployment parameter: it is what the light client is");
        println!("constructed with, so a key that moves invalidates every proof already made");
        println!("against the old one. Update RUNBOOK.md and the checklist in the same change,");
        println!("and say in the commit message what moved it.");

        return;
    }

    println!("The verifying key moved for: {}", drifted.join(", "));
    println!();
    println!("This is not a lint failure to be silenced. The key is what the light client pins at");
    println!("construction, so it moving means the guest, its patches, or the SP1 version");
    println!("changed, and any proof made against the old key no longer verifies.");
    println!();
    println!(
        "If the change was deliberate: cargo run -p sp1-stellar-prover --bin vkeys -- --write"
    );

    std::process::exit(1);
}

fn read_pinned(path: &std::path::Path, write: bool) -> BTreeMap<String, String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display())),
        Err(_) if write => BTreeMap::new(),
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
