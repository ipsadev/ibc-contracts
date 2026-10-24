// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use alloy_sol_types::SolValue;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use stellar_light_client_membership::membership;
use stellar_light_client_membership::scenario::client_state;
use stellar_light_client_membership::scenario::entry;
use stellar_light_client_membership::scenario::proof_publishing;
use stellar_light_client_membership::scenario::Tree;
use stellar_light_client_solidity_types::MembershipOutput;

const ELF_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-membership";

fn leaves() -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..16_u32)
        .map(|index| {
            (
                format!("commitments/ports/transfer/channels/0/{index}").into_bytes(),
                format!("packet-{index}").into_bytes(),
            )
        })
        .collect()
}

fn main() {
    let root = repository_root();

    let elf_path = match std::env::var("SP1_STELLAR_MEMBERSHIP_ELF") {
        Ok(given) => PathBuf::from(given),
        Err(_) => root.join(ELF_PATH),
    };

    let pairs = match std::env::args().nth(1) {
        Some(given) => given.parse().expect("the pair count must be a number"),
        None => 1_usize,
    };

    let tree = Tree::build(&leaves());

    let (state_root_proof, result_hash) = proof_publishing(tree.root);

    let state = client_state();

    let mut entries = Vec::with_capacity(pairs);

    for index in 0..pairs {
        let key = format!("commitments/ports/transfer/channels/0/{index}").into_bytes();
        let value = format!("packet-{index}").into_bytes();

        entries.push(entry(&tree, &key, &value));
    }

    let native = membership(&state, result_hash, &state_root_proof, &entries)
        .expect("native membership failed");

    let elf_bytes = std::fs::read(&elf_path).unwrap_or_else(|error| {
        panic!(
            "cannot read the membership ELF at {}: {error}\nbuild it with: cd ibc-solidity/programs/sp1-stellar-programs/membership && cargo prove build --locked",
            elf_path.display()
        )
    });

    println!(
        "guest ELF {} ({} bytes)",
        elf_path.display(),
        elf_bytes.len()
    );
    println!("leaves    {}", leaves().len());
    println!("pairs     {pairs}");
    println!("ibc root  {}", hex::encode(tree.root));
    println!();

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&state.abi_encode());
    stdin.write_slice(&result_hash);
    stdin.write_slice(&state_root_proof.abi_encode());
    stdin.write_slice(&(pairs as u16).to_le_bytes());

    for one in &entries {
        stdin.write_slice(&one.abi_encode());
    }

    let client = ProverClient::builder().cpu().build();

    let (public_values, report) = client
        .execute(Elf::from(elf_bytes), stdin)
        .run()
        .expect("the membership program failed to execute");

    println!("cycles    {}", report.total_instruction_count());
    println!();

    let output = MembershipOutput::abi_decode(public_values.as_slice())
        .expect("the committed public values are not a MembershipOutput");

    let mut failures = Vec::new();

    compare(
        &mut failures,
        "ibc root",
        output.ibcRoot.0,
        native.ibcRoot.0,
    );
    compare(
        &mut failures,
        "ibc root against the tree the scenario built",
        output.ibcRoot.0,
        tree.root,
    );
    compare(
        &mut failures,
        "transaction set result hash",
        output.transactionSetResultHash.0,
        result_hash,
    );
    compare(
        &mut failures,
        "router contract id echo",
        output.clientState.routerContractId.0,
        state.routerContractId.0,
    );
    compare(
        &mut failures,
        "pairs returned",
        output.kvPairs.len(),
        entries.len(),
    );

    for (index, pair) in output.kvPairs.iter().enumerate() {
        compare(
            &mut failures,
            &format!("pair {index} value"),
            pair.value.to_vec(),
            entries[index].kvPair.value.to_vec(),
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
