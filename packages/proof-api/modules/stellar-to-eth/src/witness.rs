use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};
use sp1_stellar_prover::{Fixture, PacketWitness};
use stellar_xdr::{
    ContractEventBody, InvokeHostFunctionResult, InvokeHostFunctionSuccessPreImage,
    LedgerEntryData, Limits, OperationResult, OperationResultTr, ReadXdr, ScAddress, ScVal,
    TransactionResultPair, TransactionResultResult, WriteXdr,
};

use crate::soroban::{Executed, Transaction};

const TREE_DEPTH: usize = 64;
const EMPTY: [u8; 32] = [0; 32];

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn leaf_hash(key: &[u8], value: &[u8]) -> [u8; 32] {
    let mut preimage = vec![0_u8];

    preimage.extend_from_slice(&sha256(key));
    preimage.extend_from_slice(&sha256(value));

    sha256(&preimage)
}

fn inner_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    if *left == EMPTY && *right == EMPTY {
        return EMPTY;
    }

    let mut preimage = vec![1_u8];

    preimage.extend_from_slice(left);
    preimage.extend_from_slice(right);

    sha256(&preimage)
}

fn key_index(key: &[u8]) -> u64 {
    let digest = sha256(key);
    let mut prefix = [0_u8; 8];

    prefix.copy_from_slice(&digest[..8]);

    u64::from_be_bytes(prefix)
}

fn fold(index: u64, leaf: [u8; 32], siblings: &[[u8; 32]]) -> [u8; 32] {
    let mut current = leaf;
    let mut remaining = index;

    for sibling in siblings {
        current = if remaining & 1 == 0 {
            inner_hash(&current, sibling)
        } else {
            inner_hash(sibling, &current)
        };

        remaining >>= 1;
    }

    current
}

fn success_of(pair: &TransactionResultPair) -> Result<[u8; 32]> {
    let TransactionResultResult::TxSuccess(operations) = &pair.result.result else {
        bail!("the archived result is not a plain transaction success");
    };

    match operations.first() {
        Some(OperationResult::OpInner(OperationResultTr::InvokeHostFunction(
            InvokeHostFunctionResult::Success(hash),
        ))) => Ok(hash.0),
        _ => bail!("the archived result's first operation is not a successful host function"),
    }
}

pub fn attach(
    fixture: &mut Fixture,
    transaction: &Transaction,
    executed: &Executed,
    router: [u8; 32],
    path: &[u8],
    siblings: &[[u8; 32]],
) -> Result<Vec<u8>> {
    if u64::from(transaction.ledger) != fixture.ledger_seq {
        bail!(
            "transaction {} is in ledger {}, the fixture proves {}",
            transaction.hash,
            transaction.ledger,
            fixture.ledger_seq
        );
    }

    let result_index = transaction
        .application_order
        .checked_sub(1)
        .ok_or_else(|| anyhow!("application order 0 is not a transaction position"))?;
    let pair_hex = fixture
        .result_pairs_xdr
        .get(result_index as usize)
        .ok_or_else(|| anyhow!("the fixture has no result pair at {result_index}"))?;
    let pair_bytes = hex::decode(pair_hex).context("the result pair is not hex")?;
    let pair = TransactionResultPair::from_xdr(&pair_bytes, Limits::none())
        .context("decoding the archived result pair")?;

    if hex::encode(pair.transaction_hash.0) != transaction.hash {
        bail!(
            "result pair {result_index} is for {}, not {}",
            hex::encode(pair.transaction_hash.0),
            transaction.hash
        );
    }

    let success = success_of(&pair)?;
    let preimage = InvokeHostFunctionSuccessPreImage {
        return_value: executed.return_value.clone(),
        events: executed
            .events
            .clone()
            .try_into()
            .map_err(|_| anyhow!("too many events for a success preimage"))?,
    };
    let preimage_xdr = preimage
        .to_xdr(Limits::none())
        .context("encoding the success preimage")?;

    if sha256(&preimage_xdr) != success {
        bail!("the preimage rebuilt from the transaction meta does not hash to the archived success value");
    }

    let roots: Vec<[u8; 32]> = executed
        .events
        .iter()
        .filter(|event| {
            event
                .contract_id
                .as_ref()
                .is_some_and(|contract| contract.0 .0 == router)
        })
        .filter_map(|event| {
            let ContractEventBody::V0(body) = &event.body;

            match (body.topics.as_slice(), &body.data) {
                ([ScVal::Symbol(symbol)], ScVal::Bytes(root))
                    if symbol.to_utf8_string_lossy() == "ibc_root" =>
                {
                    root.as_slice().try_into().ok()
                }
                _ => None,
            }
        })
        .collect();

    let [ibc_root] = roots.as_slice() else {
        bail!(
            "expected exactly one ibc_root event from the router, found {}",
            roots.len()
        );
    };

    let mut commitments = HashMap::new();

    for entry in &executed.written {
        let LedgerEntryData::ContractData(data) = &entry.data else {
            continue;
        };

        let ScAddress::Contract(contract) = &data.contract else {
            continue;
        };

        if contract.0 .0 != router {
            continue;
        }

        if let (ScVal::Bytes(key), ScVal::Bytes(value)) = (&data.key, &data.val) {
            commitments.insert(key.to_vec(), value.to_vec());
        }
    }

    let value = commitments
        .get(path)
        .cloned()
        .ok_or_else(|| anyhow!("the transaction did not write path {}", hex::encode(path)))?;

    if siblings.len() != TREE_DEPTH {
        bail!("expected {TREE_DEPTH} siblings, got {}", siblings.len());
    }

    if fold(key_index(path), leaf_hash(path, &value), siblings) != *ibc_root {
        bail!("the siblings do not fold the written value to the published ibc_root");
    }

    fixture.packet = Some(PacketWitness {
        transaction_hash: transaction.hash.clone(),
        router_contract_id: hex::encode(router),
        result_index,
        success_preimage_xdr: hex::encode(&preimage_xdr),
        ibc_root: hex::encode(ibc_root),
        path: hex::encode(path),
        value: hex::encode(&value),
        siblings: siblings.iter().map(hex::encode).collect(),
    });

    Ok(value)
}
