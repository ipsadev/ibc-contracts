// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use crate::smt;
use sha2::Digest;
use sha2::Sha256;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::Height;
use stellar_light_client_solidity_types::KVPair;
use stellar_light_client_solidity_types::MembershipProofEntry;
use stellar_light_client_solidity_types::StateRootProof;
use stellar_light_client_solidity_types::SupportedZkAlgorithm;

pub const ROUTER_ID: [u8; 32] = [0x5a; 32];
pub const ROOT_TOPIC: &[u8] = b"ibc_root";

pub fn u32be(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

pub fn var_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = u32be(data.len() as u32).to_vec();

    out.extend_from_slice(data);
    out.extend(std::iter::repeat_n(0_u8, (4 - data.len() % 4) % 4));

    out
}

pub fn scval_void() -> Vec<u8> {
    u32be(1).to_vec()
}

pub fn scval_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = u32be(13).to_vec();

    out.extend_from_slice(&var_bytes(data));

    out
}

pub fn scval_symbol(symbol: &[u8]) -> Vec<u8> {
    let mut out = u32be(15).to_vec();

    out.extend_from_slice(&var_bytes(symbol));

    out
}

pub fn contract_event(contract_id: &[u8; 32], topic: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = u32be(0).to_vec();

    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(contract_id);
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&scval_symbol(topic));
    out.extend_from_slice(data);

    out
}

pub fn success_preimage(events: &[Vec<u8>]) -> Vec<u8> {
    let mut out = scval_void();

    out.extend_from_slice(&u32be(events.len() as u32));

    for event in events {
        out.extend_from_slice(event);
    }

    out
}

pub fn invoke_result_pair(transaction_hash: [u8; 32], success_hash: [u8; 32]) -> Vec<u8> {
    let mut out = transaction_hash.to_vec();

    out.extend_from_slice(&0_u64.to_be_bytes());
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(24));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&success_hash);
    out.extend_from_slice(&u32be(0));

    out
}

pub fn result_set_hash(pairs: &[Vec<u8>]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(u32be(pairs.len() as u32));

    for pair in pairs {
        hasher.update(pair);
    }

    hasher.finalize().into()
}

pub fn state_root_proof_for(events: &[Vec<u8>]) -> (StateRootProof, [u8; 32]) {
    let preimage = success_preimage(events);

    let success_hash: [u8; 32] = Sha256::digest(&preimage).into();

    let pairs = vec![
        invoke_result_pair([0x11; 32], [0x99; 32]),
        invoke_result_pair([0x22; 32], success_hash),
    ];

    let hash = result_set_hash(&pairs);

    let proof = StateRootProof {
        resultPairs: pairs.into_iter().map(Into::into).collect(),
        resultIndex: 1,
        successPreimageXdr: preimage.into(),
    };

    (proof, hash)
}

pub fn proof_publishing(root: [u8; 32]) -> (StateRootProof, [u8; 32]) {
    state_root_proof_for(&[contract_event(&ROUTER_ID, ROOT_TOPIC, &scval_bytes(&root))])
}

pub struct Tree {
    levels: Vec<BTreeMap<u64, [u8; 32]>>,
    pub root: [u8; 32],
}

impl Tree {
    pub fn build(entries: &[(Vec<u8>, Vec<u8>)]) -> Self {
        let mut level: BTreeMap<u64, [u8; 32]> = BTreeMap::new();

        for (key, value) in entries {
            level.insert(
                smt::key_index(key),
                smt::leaf_hash(smt::sha256(key), smt::sha256(value)),
            );
        }

        let mut levels = vec![level.clone()];

        for _ in 0..smt::TREE_DEPTH {
            let mut next: BTreeMap<u64, [u8; 32]> = BTreeMap::new();

            for index in level.keys().copied().collect::<Vec<_>>() {
                let parent = index >> 1;

                if next.contains_key(&parent) {
                    continue;
                }

                let left = level.get(&(parent << 1)).copied().unwrap_or(smt::EMPTY);
                let right = level
                    .get(&((parent << 1) | 1))
                    .copied()
                    .unwrap_or(smt::EMPTY);

                next.insert(parent, smt::inner_hash(left, right));
            }

            level = next;
            levels.push(level.clone());
        }

        let root = level.get(&0).copied().unwrap_or(smt::EMPTY);

        Self { levels, root }
    }

    pub fn siblings(&self, key: &[u8]) -> Vec<[u8; 32]> {
        let mut index = smt::key_index(key);

        let mut out = Vec::with_capacity(smt::TREE_DEPTH);

        for depth in 0..smt::TREE_DEPTH {
            let sibling = self.levels[depth]
                .get(&(index ^ 1))
                .copied()
                .unwrap_or(smt::EMPTY);

            out.push(sibling);
            index >>= 1;
        }

        out
    }
}

pub fn entry(tree: &Tree, key: &[u8], value: &[u8]) -> MembershipProofEntry {
    MembershipProofEntry {
        kvPair: KVPair {
            path: vec![key.to_vec().into()],
            value: value.to_vec().into(),
        },
        siblings: tree.siblings(key).into_iter().map(Into::into).collect(),
    }
}

pub fn client_state() -> ClientState {
    ClientState {
        networkId: [0x01_u8; 32].into(),
        trustRootFingerprint: [0x02_u8; 32].into(),
        routerContractId: ROUTER_ID.into(),
        rootEventTopic: ROOT_TOPIC.to_vec().into(),
        latestHeight: Height {
            revisionNumber: 0,
            revisionHeight: 100,
        },
        isFrozen: false,
        zkAlgorithm: SupportedZkAlgorithm::Groth16,
    }
}
