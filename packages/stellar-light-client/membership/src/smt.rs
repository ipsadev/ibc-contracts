// SPDX-License-Identifier: Apache-2.0

use sha2::Digest;
use sha2::Sha256;

pub const HASH_SIZE: usize = 32;
pub const TREE_DEPTH: usize = 64;
pub const EMPTY: [u8; HASH_SIZE] = [0_u8; HASH_SIZE];

pub fn sha256(data: &[u8]) -> [u8; HASH_SIZE] {
    Sha256::digest(data).into()
}

pub fn key_index(key: &[u8]) -> u64 {
    let digest = sha256(key);

    u64::from_be_bytes(digest[..8].try_into().expect("sha256 has 32 bytes"))
}

pub fn leaf_hash(key_hash: [u8; HASH_SIZE], value_hash: [u8; HASH_SIZE]) -> [u8; HASH_SIZE] {
    let mut hasher = Sha256::new();

    hasher.update([0x00]);
    hasher.update(key_hash);
    hasher.update(value_hash);

    hasher.finalize().into()
}

pub fn inner_hash(left: [u8; HASH_SIZE], right: [u8; HASH_SIZE]) -> [u8; HASH_SIZE] {
    if left == EMPTY && right == EMPTY {
        return EMPTY;
    }

    let mut hasher = Sha256::new();

    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);

    hasher.finalize().into()
}

pub fn fold_siblings(
    index: u64,
    leaf: [u8; HASH_SIZE],
    siblings: &[[u8; HASH_SIZE]],
) -> [u8; HASH_SIZE] {
    let mut current = leaf;
    let mut remaining = index;

    for sibling in siblings {
        current = if remaining & 1 == 0 {
            inner_hash(current, *sibling)
        } else {
            inner_hash(*sibling, current)
        };

        remaining >>= 1;
    }

    current
}

pub fn verify_membership(
    root: &[u8; HASH_SIZE],
    key: &[u8],
    value: &[u8],
    siblings: &[[u8; HASH_SIZE]],
) -> bool {
    if value.is_empty() || siblings.len() != TREE_DEPTH {
        return false;
    }

    let leaf = leaf_hash(sha256(key), sha256(value));

    fold_siblings(key_index(key), leaf, siblings) == *root
}

pub fn verify_non_membership(
    root: &[u8; HASH_SIZE],
    key: &[u8],
    siblings: &[[u8; HASH_SIZE]],
) -> bool {
    if siblings.len() != TREE_DEPTH {
        return false;
    }

    fold_siblings(key_index(key), EMPTY, siblings) == *root
}
