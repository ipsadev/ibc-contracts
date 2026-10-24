// SPDX-License-Identifier: Apache-2.0

use alloy_sol_types::SolValue;
use anyhow::{anyhow, Context, Result};
use sp1_sdk::{
    blocking::{Elf, NetworkProver, ProveRequest, Prover, ProverClient, SP1Stdin},
    network::proto::types::ProofRequest,
    HashableKey, ProvingKey, SP1ProofMode,
};
use stellar_light_client_membership::membership;
use stellar_light_client_solidity_types::MembershipOutput;
use stellar_light_client_update_client::{update_client, ConsensusState, UpdateClientOutput};

use crate::{decode_hex32, Fixture};

pub struct Programs {
    pub update_client: Vec<u8>,
    pub membership: Vec<u8>,
}

pub struct Trusted {
    pub height: u64,
    pub consensus_state: ConsensusState,
}

pub struct Proof {
    pub vkey: [u8; 32],
    pub public_values: Vec<u8>,
    pub proof: Vec<u8>,
    pub request_id: [u8; 32],
    pub request: Option<Box<ProofRequest>>,
}

pub struct Native {
    pub update: UpdateClientOutput,
    pub membership: MembershipOutput,
    update_stdin: SP1Stdin,
    membership_stdin: SP1Stdin,
}

pub struct RelayProofs {
    pub update: Proof,
    pub membership: Proof,
    pub update_output: UpdateClientOutput,
    pub membership_output: MembershipOutput,
}

pub fn run_natively(
    fixture: &Fixture,
    trusted: &Trusted,
    router: [u8; 32],
    time: u128,
) -> Result<Native> {
    let witness = fixture.witness_for_router(
        time,
        trusted.height,
        trusted.consensus_state.clone(),
        router,
    );

    let update = update_client(
        &witness.client_state,
        &witness.trusted_consensus_state,
        &witness.proposed_header,
        witness.time,
    )
    .map_err(|error| {
        anyhow!(
            "update-client refused ledger {}: {error:?}",
            fixture.ledger_seq
        )
    })?;

    let result_hash = update.newConsensusState.transactionSetResultHash.0;
    let state_root_proof = fixture.state_root_proof();
    let entry = fixture.membership_entry();

    let proven = membership(
        &update.clientState,
        result_hash,
        &state_root_proof,
        std::slice::from_ref(&entry),
    )
    .map_err(|error| anyhow!("membership refused the witness: {error:?}"))?;

    let published = decode_hex32(&fixture.packet_witness().ibc_root);

    if proven.ibcRoot.0 != published {
        return Err(anyhow!(
            "membership derived ibc_root {} but the router published {}",
            hex::encode(proven.ibcRoot.0),
            hex::encode(published)
        ));
    }

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
    membership_stdin.write_slice(&entry.abi_encode());

    Ok(Native {
        update,
        membership: proven,
        update_stdin,
        membership_stdin,
    })
}

pub fn prove_on_network(native: Native, programs: &Programs) -> Result<RelayProofs> {
    let client = ProverClient::builder().network().build();

    let update = prove_one(
        &client,
        &programs.update_client,
        native.update_stdin,
        "update-client",
    )?;

    let membership = prove_one(
        &client,
        &programs.membership,
        native.membership_stdin,
        "membership",
    )?;

    if update.public_values != native.update.abi_encode() {
        return Err(anyhow!(
            "the update-client proof committed values the native run did not"
        ));
    }

    if membership.public_values != native.membership.abi_encode() {
        return Err(anyhow!(
            "the membership proof committed values the native run did not"
        ));
    }

    Ok(RelayProofs {
        update,
        membership,
        update_output: native.update,
        membership_output: native.membership,
    })
}

fn prove_one(client: &NetworkProver, elf: &[u8], stdin: SP1Stdin, name: &str) -> Result<Proof> {
    let proving_key = client
        .setup(Elf::from(elf.to_vec()))
        .map_err(|error| anyhow!("the {name} ELF could not be set up: {error}"))?;

    let verifying_key = proving_key.verifying_key().clone();

    let request_id = client
        .prove(&proving_key, stdin)
        .mode(SP1ProofMode::Groth16)
        .request()
        .map_err(|error| anyhow!("requesting the {name} proof failed: {error}"))?;

    let proof = client
        .wait_proof(request_id, None, None)
        .map_err(|error| anyhow!("proving {name} failed (request {request_id}): {error}"))?;

    client
        .verify(&proof, &verifying_key, None)
        .map_err(|error| anyhow!("the {name} proof did not verify: {error}"))?;

    let request = client
        .get_proof_request(request_id)
        .ok()
        .flatten()
        .map(Box::new);

    let vkey = hex::decode(verifying_key.bytes32().trim_start_matches("0x"))
        .context("the verifying key is not hex")?
        .try_into()
        .map_err(|_| anyhow!("the verifying key is not 32 bytes"))?;

    Ok(Proof {
        vkey,
        public_values: proof.public_values.as_slice().to_vec(),
        proof: proof.bytes(),
        request_id: request_id.0,
        request,
    })
}

pub struct Executed {
    pub update_cycles: u64,
    pub membership_cycles: u64,
}

pub fn execute_on_cpu(native: Native, programs: &Programs) -> Result<Executed> {
    let client = ProverClient::builder().cpu().build();

    let (update_values, update_report) = client
        .execute(
            Elf::from(programs.update_client.clone()),
            native.update_stdin,
        )
        .run()
        .map_err(|error| anyhow!("the update-client program failed to execute: {error}"))?;

    let (membership_values, membership_report) = client
        .execute(
            Elf::from(programs.membership.clone()),
            native.membership_stdin,
        )
        .run()
        .map_err(|error| anyhow!("the membership program failed to execute: {error}"))?;

    if update_values.as_slice() != native.update.abi_encode().as_slice() {
        return Err(anyhow!(
            "the update-client guest and the native run disagree"
        ));
    }

    if membership_values.as_slice() != native.membership.abi_encode().as_slice() {
        return Err(anyhow!("the membership guest and the native run disagree"));
    }

    Ok(Executed {
        update_cycles: update_report.total_instruction_count(),
        membership_cycles: membership_report.total_instruction_count(),
    })
}

pub fn verifying_key(elf: &[u8]) -> Result<[u8; 32]> {
    let cpu = ProverClient::builder().cpu().build();
    let proving_key = cpu
        .setup(Elf::from(elf.to_vec()))
        .map_err(|error| anyhow!("the ELF could not be set up: {error}"))?;

    hex::decode(
        proving_key
            .verifying_key()
            .bytes32()
            .trim_start_matches("0x"),
    )
    .context("the verifying key is not hex")?
    .try_into()
    .map_err(|_| anyhow!("the verifying key is not 32 bytes"))
}

pub struct MembershipOnly {
    pub membership: MembershipOutput,
    stdin: SP1Stdin,
}

pub fn run_membership_natively(
    fixture: &Fixture,
    stored: &ConsensusState,
    router: [u8; 32],
) -> Result<MembershipOnly> {
    let witness = fixture.witness_for_router(0, fixture.ledger_seq, stored.clone(), router);
    let result_hash = stored.transactionSetResultHash.0;
    let state_root_proof = fixture.state_root_proof();
    let entry = fixture.membership_entry();

    let proven = membership(
        &witness.client_state,
        result_hash,
        &state_root_proof,
        std::slice::from_ref(&entry),
    )
    .map_err(|error| anyhow!("membership refused the witness: {error:?}"))?;

    let published = decode_hex32(&fixture.packet_witness().ibc_root);

    if proven.ibcRoot.0 != published {
        return Err(anyhow!(
            "membership derived ibc_root {} but the router published {}",
            hex::encode(proven.ibcRoot.0),
            hex::encode(published)
        ));
    }

    let mut stdin = SP1Stdin::new();

    stdin.write_slice(&witness.client_state.abi_encode());
    stdin.write_slice(&result_hash);
    stdin.write_slice(&state_root_proof.abi_encode());
    stdin.write_slice(&1_u16.to_le_bytes());
    stdin.write_slice(&entry.abi_encode());

    Ok(MembershipOnly {
        membership: proven,
        stdin,
    })
}

pub fn execute_membership_on_cpu(native: MembershipOnly, programs: &Programs) -> Result<u64> {
    let client = ProverClient::builder().cpu().build();

    let (values, report) = client
        .execute(Elf::from(programs.membership.clone()), native.stdin)
        .run()
        .map_err(|error| anyhow!("the membership program failed to execute: {error}"))?;

    if values.as_slice() != native.membership.abi_encode().as_slice() {
        return Err(anyhow!("the membership guest and the native run disagree"));
    }

    Ok(report.total_instruction_count())
}

pub fn prove_membership_on_network(native: MembershipOnly, programs: &Programs) -> Result<Proof> {
    let client = ProverClient::builder().network().build();
    let proof = prove_one(&client, &programs.membership, native.stdin, "membership")?;

    if proof.public_values != native.membership.abi_encode() {
        return Err(anyhow!(
            "the membership proof committed values the native run did not"
        ));
    }

    Ok(proof)
}
