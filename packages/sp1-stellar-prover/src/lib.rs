// SPDX-License-Identifier: Apache-2.0

pub mod relay;

use std::path::Path;

use ipsa_scp_verifier::steps::verify_header;
use ipsa_scp_verifier::steps::HeaderInput;
use ipsa_scp_verifier::steps::QuorumConfig;
use ipsa_scp_verifier::steps::VerifiedLedger;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use stellar_light_client_membership::KVPair;
use stellar_light_client_membership::MembershipProofEntry;
use stellar_light_client_solidity_types::StateRootProof;
use stellar_light_client_update_client::ClientState;
use stellar_light_client_update_client::ConsensusState;
use stellar_light_client_update_client::Header;
use stellar_light_client_update_client::Height;
use stellar_light_client_update_client::SupportedZkAlgorithm;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    #[serde(default)]
    pub network: String,
    pub ledger_seq: u64,
    pub network_id: String,
    pub ledger_hash: String,
    pub close_time: u64,
    pub tx_set_result_hash: String,
    #[serde(default)]
    pub bucket_list_hash: String,
    pub ledger_header_xdr: String,
    pub quorum_sets_xdr: Vec<String>,
    pub envelopes_xdr: Vec<String>,
    #[serde(default)]
    pub next_envelopes_xdr: Vec<String>,
    #[serde(default)]
    pub next_tx_set_xdr: String,
    #[serde(default)]
    pub result_pairs_xdr: Vec<String>,
    #[serde(default)]
    pub packet: Option<PacketWitness>,
}

#[derive(Debug, Deserialize)]
pub struct PacketWitness {
    pub transaction_hash: String,
    pub router_contract_id: String,
    pub result_index: u32,
    pub success_preimage_xdr: String,
    pub ibc_root: String,
    pub path: String,
    pub value: String,
    pub siblings: Vec<String>,
}

pub const ROUTER_CONTRACT_ID: [u8; 32] = [0x5a; 32];
pub const ROOT_EVENT_TOPIC: &[u8] = b"ibc_root";
pub const ROUTER_CONTRACT_ID_VARIABLE: &str = "ROUTER_CONTRACT_ID";

const CONTRACT_STRKEY_VERSION: u8 = 2 << 3;
const BASE32: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub struct Witness {
    pub client_state: ClientState,
    pub trusted_consensus_state: ConsensusState,
    pub proposed_header: Header,
    pub time: u128,
}

impl Fixture {
    pub fn load(path: &Path) -> Self {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

        serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
    }

    pub fn from_json(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    pub fn trust_root_xdr(&self) -> Vec<u8> {
        let first = self
            .quorum_sets_xdr
            .first()
            .expect("the fixture carries no quorum sets");

        decode_hex(first)
    }

    pub fn header_input(&self, trusted_previous_hash: Option<[u8; 32]>) -> HeaderInput {
        HeaderInput {
            slot: self.ledger_seq,
            network_id: decode_hex32(&self.network_id),
            ledger_header_xdr: decode_hex(&self.ledger_header_xdr),
            envelopes_xdr: self
                .envelopes_xdr
                .iter()
                .map(|one| decode_hex(one))
                .collect(),
            quorum_sets_xdr: self
                .quorum_sets_xdr
                .iter()
                .map(|one| decode_hex(one))
                .collect(),
            result_pairs_xdr: Vec::new(),
            recorded_ledger_hash: None,
            trusted_previous_hash,
        }
    }

    pub fn verify_natively(&self, trusted_previous_hash: Option<[u8; 32]>) -> VerifiedLedger {
        let trust_root_xdr = self.trust_root_xdr();

        let config = QuorumConfig::from_xdr(&trust_root_xdr).expect("the trust root is unusable");

        let input = self.header_input(trusted_previous_hash);

        verify_header(&input, &config).expect("native verification failed")
    }

    pub fn witness(&self, verified: &VerifiedLedger, time: u128) -> Witness {
        let trusted = ConsensusState {
            closeTime: 0,
            ledgerHash: verified.previous_ledger_hash.into(),
            previousLedgerHash: [0_u8; 32].into(),
            bucketListHash: [0_u8; 32].into(),
            transactionSetResultHash: [0_u8; 32].into(),
        };

        self.witness_with_trusted(time, trusted)
    }

    pub fn witness_with_trusted(
        &self,
        time: u128,
        trusted_consensus_state: ConsensusState,
    ) -> Witness {
        self.witness_at(time, self.ledger_seq - 1, trusted_consensus_state)
    }

    pub fn witness_at(
        &self,
        time: u128,
        trusted_height: u64,
        trusted_consensus_state: ConsensusState,
    ) -> Witness {
        self.witness_for_router(
            time,
            trusted_height,
            trusted_consensus_state,
            router_contract_id_from_environment().unwrap_or(ROUTER_CONTRACT_ID),
        )
    }

    pub fn witness_for_router(
        &self,
        time: u128,
        trusted_height: u64,
        trusted_consensus_state: ConsensusState,
        router: [u8; 32],
    ) -> Witness {
        let trust_root_xdr = self.trust_root_xdr();

        let fingerprint: [u8; 32] = Sha256::digest(&trust_root_xdr).into();

        let client_state = ClientState {
            networkId: decode_hex32(&self.network_id).into(),
            trustRootFingerprint: fingerprint.into(),
            routerContractId: router.into(),
            rootEventTopic: ROOT_EVENT_TOPIC.to_vec().into(),
            latestHeight: Height {
                revisionNumber: 0,
                revisionHeight: trusted_height,
            },
            isFrozen: false,
            zkAlgorithm: SupportedZkAlgorithm::Groth16,
        };

        let proposed_header = Header {
            slot: self.ledger_seq,
            ledgerHeaderXdr: decode_hex(&self.ledger_header_xdr).into(),
            envelopesXdr: self
                .envelopes_xdr
                .iter()
                .map(|one| decode_hex(one).into())
                .collect(),
            quorumSetsXdr: self
                .quorum_sets_xdr
                .iter()
                .map(|one| decode_hex(one).into())
                .collect(),
            trustRootXdr: trust_root_xdr.into(),
            nextEnvelopesXdr: self
                .next_envelopes_xdr
                .iter()
                .map(|one| decode_hex(one).into())
                .collect(),
            nextTxSetXdr: decode_hex(&self.next_tx_set_xdr).into(),
        };

        Witness {
            client_state,
            trusted_consensus_state,
            proposed_header,
            time,
        }
    }
}

impl Fixture {
    pub fn packet_witness(&self) -> &PacketWitness {
        self.packet.as_ref().unwrap_or_else(|| {
            panic!(
                "the fixture for ledger {} carries no packet witness. Add one with \
                 packages/stellar-light-client/fixtures/fetch-packet-witness.py",
                self.ledger_seq
            )
        })
    }

    pub fn state_root_proof(&self) -> StateRootProof {
        let packet = self.packet_witness();

        StateRootProof {
            resultPairs: self
                .result_pairs_xdr
                .iter()
                .map(|one| decode_hex(one).into())
                .collect(),
            resultIndex: packet.result_index,
            successPreimageXdr: decode_hex(&packet.success_preimage_xdr).into(),
        }
    }

    pub fn membership_entry(&self) -> MembershipProofEntry {
        let packet = self.packet_witness();

        MembershipProofEntry {
            kvPair: KVPair {
                path: vec![decode_hex(&packet.path).into()],
                value: decode_hex(&packet.value).into(),
            },
            siblings: packet
                .siblings
                .iter()
                .map(|one| decode_hex32(one).into())
                .collect(),
        }
    }
}

pub fn router_contract_id_from_environment() -> Option<[u8; 32]> {
    let raw = std::env::var(ROUTER_CONTRACT_ID_VARIABLE).ok()?;

    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return None;
    }

    Some(decode_contract_id(trimmed).unwrap_or_else(|reason| {
        panic!("{ROUTER_CONTRACT_ID_VARIABLE}={trimmed} cannot be read: {reason}")
    }))
}

pub fn decode_contract_id(text: &str) -> Result<[u8; 32], String> {
    if let Some(stripped) = text.strip_prefix("0x") {
        return hex_contract_id(stripped);
    }

    if text.len() == 64 && text.chars().all(|one| one.is_ascii_hexdigit()) {
        return hex_contract_id(text);
    }

    decode_contract_strkey(text)
}

fn hex_contract_id(hexed: &str) -> Result<[u8; 32], String> {
    let raw = hex::decode(hexed).map_err(|error| format!("not hex: {error}"))?;

    raw.as_slice()
        .try_into()
        .map_err(|_| format!("expected 32 bytes, got {}", raw.len()))
}

fn decode_contract_strkey(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 56 {
        return Err(format!(
            "a contract strkey is 56 characters beginning with C, this is {}",
            text.len()
        ));
    }

    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    let mut decoded = Vec::with_capacity(35);

    for one in text.bytes() {
        let value = BASE32
            .iter()
            .position(|candidate| *candidate == one)
            .ok_or_else(|| format!("{} is not a base32 character", one as char))?;

        accumulator = (accumulator << 5) | value as u32;
        bits += 5;

        if bits >= 8 {
            bits -= 8;
            decoded.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }

    if decoded.len() != 35 {
        return Err(format!("decoded to {} bytes, expected 35", decoded.len()));
    }

    if decoded[0] != CONTRACT_STRKEY_VERSION {
        return Err(format!(
            "version byte {:#04x} is not a contract, expected {CONTRACT_STRKEY_VERSION:#04x}",
            decoded[0]
        ));
    }

    let checksum = u16::from_le_bytes([decoded[33], decoded[34]]);
    let computed = crc16_xmodem(&decoded[..33]);

    if checksum != computed {
        return Err(format!(
            "checksum {checksum:#06x} does not match {computed:#06x}, so the strkey is mistyped"
        ));
    }

    decoded[1..33]
        .try_into()
        .map_err(|_| "the payload is not 32 bytes".to_string())
}

fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;

    for byte in data {
        crc ^= (*byte as u16) << 8;

        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }

    crc
}

pub fn decode_hex(hexed: &str) -> Vec<u8> {
    hex::decode(hexed).unwrap_or_else(|error| panic!("bad hex in the fixture: {error}"))
}

pub fn decode_hex32(hexed: &str) -> [u8; 32] {
    decode_hex(hexed)
        .as_slice()
        .try_into()
        .unwrap_or_else(|_| panic!("expected 32 bytes of hex, got {}", hexed.len() / 2))
}

pub fn consensus_state_of(verified: &VerifiedLedger) -> ConsensusState {
    ConsensusState {
        closeTime: verified.close_time,
        ledgerHash: verified.ledger_hash.into(),
        previousLedgerHash: verified.previous_ledger_hash.into(),
        bucketListHash: verified.bucket_list_hash.into(),
        transactionSetResultHash: verified.tx_set_result_hash.into(),
    }
}
