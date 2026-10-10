use anyhow::{anyhow, Context, Result};
use sp1_sdk::{
    blocking::{Elf, ProveRequest, Prover, ProverClient, SP1Stdin},
    HashableKey, ProvingKey, SP1ProofMode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Network,
    Execute,
}

pub struct Proof {
    pub vkey: [u8; 32],
    pub public_values: Vec<u8>,
    pub proof: Vec<u8>,
}

pub enum Outcome {
    Proved(Proof),
    Executed { public_values: Vec<u8>, cycles: u64 },
}

impl Outcome {
    pub fn public_values(&self) -> &[u8] {
        match self {
            Self::Proved(proof) => &proof.public_values,
            Self::Executed { public_values, .. } => public_values,
        }
    }

    pub fn client_message(&self) -> Option<Vec<u8>> {
        let Self::Proved(proof) = self else {
            return None;
        };

        let length = u32::try_from(proof.public_values.len()).ok()?;
        let mut message = length.to_be_bytes().to_vec();

        message.extend_from_slice(&proof.public_values);
        message.extend_from_slice(&proof.proof);

        Some(message)
    }
}

pub fn header_stdin(
    client_state_json: &[u8],
    consensus_state_json: &[u8],
    header_json: &[u8],
    timestamp: u64,
) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();

    stdin.write_slice(client_state_json);
    stdin.write_slice(consensus_state_json);
    stdin.write_slice(header_json);
    stdin.write_slice(&timestamp.to_le_bytes());

    stdin
}

pub fn membership_stdin(
    client_state_json: &[u8],
    consensus_state_json: &[u8],
    storage_proof_json: &[u8],
    path_json: &[u8],
    value: &[u8],
) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();

    stdin.write_slice(client_state_json);
    stdin.write_slice(consensus_state_json);
    stdin.write_slice(storage_proof_json);
    stdin.write_slice(path_json);
    stdin.write_slice(value);

    stdin
}

pub fn run(elf: &[u8], stdin: SP1Stdin, mode: Mode, name: &str) -> Result<Outcome> {
    let cpu = ProverClient::builder().cpu().build();

    let (values, report) = cpu
        .execute(Elf::from(elf.to_vec()), stdin.clone())
        .run()
        .map_err(|error| anyhow!("the {name} program failed to execute: {error}"))?;

    if mode == Mode::Execute {
        return Ok(Outcome::Executed {
            public_values: values.as_slice().to_vec(),
            cycles: report.total_instruction_count(),
        });
    }

    let network = ProverClient::builder().network().build();
    let proving_key = network
        .setup(Elf::from(elf.to_vec()))
        .map_err(|error| anyhow!("the {name} ELF could not be set up: {error}"))?;
    let verifying_key = proving_key.verifying_key().clone();

    let proof = network
        .prove(&proving_key, stdin)
        .mode(SP1ProofMode::Groth16)
        .run()
        .map_err(|error| anyhow!("proving {name} failed: {error}"))?;

    network
        .verify(&proof, &verifying_key, None)
        .map_err(|error| anyhow!("the {name} proof did not verify: {error}"))?;

    if proof.public_values.as_slice() != values.as_slice() {
        return Err(anyhow!(
            "the {name} proof committed values its execution did not"
        ));
    }

    let vkey = hex::decode(verifying_key.bytes32().trim_start_matches("0x"))
        .context("the verifying key is not hex")?
        .try_into()
        .map_err(|_| anyhow!("the verifying key is not 32 bytes"))?;

    Ok(Outcome::Proved(Proof {
        vkey,
        public_values: proof.public_values.as_slice().to_vec(),
        proof: proof.bytes(),
    }))
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
