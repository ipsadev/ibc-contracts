// SPDX-License-Identifier: Apache-2.0

//! This module recovers the execution data a consensus state needs from a light client header.
//!
//! From gloas on, light client headers carry only the execution block hash, so the execution
//! header itself is passed as a witness and checked against that hash.

use alloy_primitives::{keccak256, Bytes, B256};
use alloy_rlp::{Decodable, Header as RlpHeader};
use ethereum_types::consensus::light_client_header::LightClientHeader;

use crate::error::EthereumIBCError;

const STATE_ROOT_FIELD: usize = 3;
const NUMBER_FIELD: usize = 8;
const TIMESTAMP_FIELD: usize = 11;

/// The execution data a consensus state is built from
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionData {
    /// The execution state root
    pub state_root: B256,
    /// The execution block number
    pub block_number: u64,
    /// The execution block timestamp, in seconds
    pub timestamp: u64,
}

/// Returns the execution data a light client header commits to.
///
/// Headers before gloas carry it; from gloas on it is read from `execution_header`, the RLP
/// encoded execution block header, which must hash to the header's execution block hash.
/// # Errors
/// Returns an error if a gloas header has no witness, or the witness does not match or decode.
pub fn execution_data(
    header: &LightClientHeader,
    execution_header: Option<&Bytes>,
) -> Result<ExecutionData, EthereumIBCError> {
    if let Some(execution) = header.execution() {
        return Ok(ExecutionData {
            state_root: execution.state_root,
            block_number: execution.block_number,
            timestamp: execution.timestamp,
        });
    }

    let encoded = execution_header.ok_or(EthereumIBCError::MissingExecutionHeader)?;

    execution_data_for_block_hash(header.execution_block_hash(), encoded)
}

/// Returns the execution data of the RLP encoded execution block header `encoded`, which must
/// hash to `block_hash`.
/// # Errors
/// Returns an error if the header does not hash to `block_hash` or does not decode.
pub fn execution_data_for_block_hash(
    block_hash: B256,
    encoded: &Bytes,
) -> Result<ExecutionData, EthereumIBCError> {
    let found = keccak256(encoded);

    ensure!(
        found == block_hash,
        EthereumIBCError::ExecutionHeaderHashMismatch {
            expected: block_hash,
            found
        }
    );

    decode_execution_header(encoded)
}

fn decode_execution_header(encoded: &[u8]) -> Result<ExecutionData, EthereumIBCError> {
    let mut buffer = encoded;

    let list =
        RlpHeader::decode(&mut buffer).map_err(|_| EthereumIBCError::ExecutionHeaderDecode)?;

    ensure!(
        list.list && list.payload_length == buffer.len(),
        EthereumIBCError::ExecutionHeaderDecode
    );

    let mut state_root = None;
    let mut block_number = None;
    let mut timestamp = None;

    for field in 0..=TIMESTAMP_FIELD {
        match field {
            STATE_ROOT_FIELD => state_root = Some(decode_field::<B256>(&mut buffer)?),
            NUMBER_FIELD => block_number = Some(decode_field::<u64>(&mut buffer)?),
            TIMESTAMP_FIELD => timestamp = Some(decode_field::<u64>(&mut buffer)?),
            _ => skip_field(&mut buffer)?,
        }
    }

    Ok(ExecutionData {
        state_root: state_root.ok_or(EthereumIBCError::ExecutionHeaderDecode)?,
        block_number: block_number.ok_or(EthereumIBCError::ExecutionHeaderDecode)?,
        timestamp: timestamp.ok_or(EthereumIBCError::ExecutionHeaderDecode)?,
    })
}

fn decode_field<T: Decodable>(buffer: &mut &[u8]) -> Result<T, EthereumIBCError> {
    T::decode(buffer).map_err(|_| EthereumIBCError::ExecutionHeaderDecode)
}

fn skip_field(buffer: &mut &[u8]) -> Result<(), EthereumIBCError> {
    let field = RlpHeader::decode(buffer).map_err(|_| EthereumIBCError::ExecutionHeaderDecode)?;

    ensure!(
        field.payload_length <= buffer.len(),
        EthereumIBCError::ExecutionHeaderDecode
    );

    *buffer = &buffer[field.payload_length..];

    Ok(())
}
