// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use alloy_sol_types::SolValue;
use ipsa_scp_verifier::evaluate::apply_mutation;
use ipsa_scp_verifier::evaluate::MUTATIONS;
use ipsa_scp_verifier::steps::verify_header;
use ipsa_scp_verifier::steps::HeaderInput;
use ipsa_scp_verifier::steps::QuorumConfig;
use sha2::Digest;
use sha2::Sha256;
use sp1_sdk::blocking::Elf;
use sp1_sdk::blocking::Prover;
use sp1_sdk::blocking::ProverClient;
use sp1_sdk::blocking::SP1Stdin;
use sp1_stellar_prover::Fixture;
use sp1_stellar_prover::ROOT_EVENT_TOPIC;
use sp1_stellar_prover::ROUTER_CONTRACT_ID;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::ConsensusState;
use stellar_light_client_solidity_types::Header;
use stellar_light_client_solidity_types::Height;
use stellar_light_client_solidity_types::SupportedZkAlgorithm;
use stellar_light_client_update_client::update_client;

const ELF_PATH: &str = "ibc-solidity/programs/sp1-stellar-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-stellar-update-client";
const FIXTURE_PATH: &str = "packages/stellar-light-client/fixtures/mainnet-63907880.json";
const VERIFICATION_TIME: u128 = 1_700_000_000_000_000_000;

const OWNED_ELSEWHERE: [(&str, &str); 1] = [(
    "tampered-result-pairs",
    "the membership program, which binds result pairs through verify_state_root",
)];

#[derive(PartialEq, Eq)]
enum Verdict {
    Rejected,
    Accepted,
}

impl Verdict {
    fn label(&self) -> &'static str {
        match self {
            Self::Rejected => "rejected",
            Self::Accepted => "ACCEPTED",
        }
    }
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "off");
    }

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

    let elf_bytes = std::fs::read(&elf_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", elf_path.display()));

    let next_envelopes: Vec<Vec<u8>> = fixture
        .next_envelopes_xdr
        .iter()
        .map(|one| sp1_stellar_prover::decode_hex(one))
        .collect();

    let next_tx_set = sp1_stellar_prover::decode_hex(&fixture.next_tx_set_xdr);

    let trust_root_xdr = fixture.trust_root_xdr();

    let honest_config =
        QuorumConfig::from_xdr(&trust_root_xdr).expect("the fixture's trust root is unusable");

    let honest_input = fixture.header_input(None);

    let baseline = verify_header(&honest_input, &honest_config)
        .expect("the unmutated fixture must verify natively");

    let previous_ledger_hash = baseline.previous_ledger_hash;

    println!("fixture   {}", fixture_path.display());
    println!("network   {}", fixture.network);
    println!("slot      {}", fixture.ledger_seq);
    println!("cases     {}", MUTATIONS.len());
    println!();

    let client = ProverClient::builder().cpu().build();

    let mut disagreements = Vec::new();
    let mut accepted_by_both = Vec::new();
    let mut only_with_extra_steps = Vec::new();
    let mut elsewhere = Vec::new();
    let mut skipped = 0_usize;

    for (name, step) in MUTATIONS {
        if let Some((_, owner)) = OWNED_ELSEWHERE.iter().find(|(case, _)| *case == name) {
            println!("  n/a  {name:<26} {step:<26} belongs to {owner}");
            elsewhere.push(format!("{name} ({step}) is covered by {owner}"));
            continue;
        }

        let mutated = match apply_mutation(name, &honest_input) {
            Ok(mutated) => mutated,
            Err(reason) => {
                println!("  skip {name:<26} {step:<26} not applicable: {reason}");
                skipped += 1;
                continue;
            }
        };

        let config = mutated
            .config
            .unwrap_or_else(|| QuorumConfig::from_xdr(&trust_root_xdr).expect("trust root"));

        let bare = match verify_header(&mutated.input, &config) {
            Ok(_) => Verdict::Accepted,
            Err(_) => Verdict::Rejected,
        };

        let (client_state, trusted, header) = witness(
            &mutated.input,
            &config,
            previous_ledger_hash,
            &next_envelopes,
            &next_tx_set,
        );

        let native = match update_client(&client_state, &trusted, &header, VERIFICATION_TIME) {
            Ok(_) => Verdict::Accepted,
            Err(_) => Verdict::Rejected,
        };

        let circuit = run_circuit(&client, &elf_bytes, &client_state, &trusted, &header);

        let agree = native == circuit;

        println!(
            "  {} {name:<26} {step:<26} verify_header {:<9} native {:<9} circuit {}",
            if agree { "ok  " } else { "FAIL" },
            bare.label(),
            native.label(),
            circuit.label()
        );

        if !agree {
            disagreements.push(format!(
                "{name} ({step}): native {}, circuit {}",
                native.label(),
                circuit.label()
            ));
        } else if native == Verdict::Accepted {
            accepted_by_both.push(format!("{name} ({step})"));
        } else if bare == Verdict::Accepted {
            only_with_extra_steps.push(format!("{name} ({step})"));
        }
    }

    println!();

    if !only_with_extra_steps.is_empty() {
        println!(
            "Caught only because this program runs steps 3 and 4, which verify_header does not:"
        );

        for case in &only_with_extra_steps {
            println!("  {case}");
        }

        println!();
    }

    if !elsewhere.is_empty() {
        println!("Not this program's to catch: update-client verifies the header, and the header");
        println!(
            "commits txSetResultHash through the ledger hash. Binding the result pairs to that"
        );
        println!("hash is the membership program's step, and it has its own tests for it:");

        for case in &elsewhere {
            println!("  {case}");
        }

        println!();
    }

    if !accepted_by_both.is_empty() {
        println!("Accepted by both, so this program does not cover them:");

        for case in &accepted_by_both {
            println!("  {case}");
        }

        println!();
    }

    if disagreements.is_empty() {
        println!(
            "{} of {} cases ran, {} owned by another program, {skipped} not applicable to this fixture: the circuit and the native verifier reach the same verdict on every one",
            MUTATIONS.len() - skipped - elsewhere.len(),
            MUTATIONS.len(),
            elsewhere.len()
        );

        return;
    }

    for one in &disagreements {
        println!("DISAGREEMENT {one}");
    }

    std::process::exit(1);
}

fn witness(
    input: &HeaderInput,
    config: &QuorumConfig,
    previous_ledger_hash: [u8; 32],
    next_envelopes: &[Vec<u8>],
    next_tx_set: &[u8],
) -> (ClientState, ConsensusState, Header) {
    let fingerprint: [u8; 32] = Sha256::digest(&config.raw).into();

    let client_state = ClientState {
        networkId: input.network_id.into(),
        trustRootFingerprint: fingerprint.into(),
        routerContractId: ROUTER_CONTRACT_ID.into(),
        rootEventTopic: ROOT_EVENT_TOPIC.to_vec().into(),
        latestHeight: Height {
            revisionNumber: 0,
            revisionHeight: input.slot.saturating_sub(1),
        },
        isFrozen: false,
        zkAlgorithm: SupportedZkAlgorithm::Groth16,
    };

    let trusted_consensus_state = ConsensusState {
        closeTime: 0,
        ledgerHash: previous_ledger_hash.into(),
        previousLedgerHash: [0_u8; 32].into(),
        bucketListHash: [0_u8; 32].into(),
        transactionSetResultHash: [0_u8; 32].into(),
    };

    let header = Header {
        slot: input.slot,
        ledgerHeaderXdr: input.ledger_header_xdr.clone().into(),
        envelopesXdr: input
            .envelopes_xdr
            .iter()
            .map(|one| one.clone().into())
            .collect(),
        quorumSetsXdr: input
            .quorum_sets_xdr
            .iter()
            .map(|one| one.clone().into())
            .collect(),
        trustRootXdr: config.raw.clone().into(),
        nextEnvelopesXdr: next_envelopes
            .iter()
            .map(|one| one.clone().into())
            .collect(),
        nextTxSetXdr: next_tx_set.to_vec().into(),
    };

    (client_state, trusted_consensus_state, header)
}

fn run_circuit(
    client: &impl Prover,
    elf_bytes: &[u8],
    client_state: &ClientState,
    trusted_consensus_state: &ConsensusState,
    header: &Header,
) -> Verdict {
    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&client_state.abi_encode());
    stdin.write_slice(&trusted_consensus_state.abi_encode());
    stdin.write_slice(&header.abi_encode());
    stdin.write_slice(&VERIFICATION_TIME.to_le_bytes());

    match client.execute(Elf::from(elf_bytes.to_vec()), stdin).run() {
        Ok((_, report)) if report.exit_code == 0 => Verdict::Accepted,
        Ok(_) | Err(_) => Verdict::Rejected,
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
