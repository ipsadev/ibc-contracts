// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { IICS02ClientMsgs } from "../../../msgs/IICS02ClientMsgs.sol";

/// @title ICS Stellar Messages
/// @notice Defines shared types for the Stellar light client implementations.
interface IICSStellarMsgs {
    /// @notice Defines the Stellar light client's client state.
    /// @param networkId SHA-256 of the Stellar network passphrase. Pinned by this contract so a
    /// proof of a testnet ledger cannot be replayed as a proof of a mainnet one.
    /// @param trustRootFingerprint SHA-256 of the XDR of the trusted quorum set. Pinned by this
    /// contract and asserted inside the circuit, so the prover supplies the bytes but cannot
    /// choose which quorum set they are.
    /// @param routerContractId The Soroban IBC router whose events are believed. Pinned here so a
    /// prover cannot point the proof at a contract of their own that emits the same event.
    /// @param rootEventTopic The event topic the router publishes its state root under, normally
    /// "ibc_root". Pinned for the same reason.
    /// @param latestHeight Latest height the client was updated to.
    /// @param isFrozen Whether the client is frozen due to misbehaviour.
    /// @param zkAlgorithm The zk algorithm supported by this contract (for the relayers).
    struct ClientState {
        bytes32 networkId;
        bytes32 trustRootFingerprint;
        bytes32 routerContractId;
        bytes rootEventTopic;
        IICS02ClientMsgs.Height latestHeight;
        bool isFrozen;
        SupportedZkAlgorithm zkAlgorithm;
    }

    /// @notice Defines the Stellar light client's consensus state at some height.
    /// @param closeTime The ledger close time in unix seconds.
    /// @param ledgerHash SHA-256 of the ledger header XDR.
    /// @param previousLedgerHash The ledger hash this header chains back to.
    /// @param bucketListHash Commits to all ledger state, including the IBC router's storage.
    /// @param transactionSetResultHash Commits to the transaction results of this ledger.
    struct ConsensusState {
        uint64 closeTime;
        bytes32 ledgerHash;
        bytes32 previousLedgerHash;
        bytes32 bucketListHash;
        bytes32 transactionSetResultHash;
    }

    /// @notice The header submitted to the update client program.
    /// @dev Every field here is supplied by the prover and is therefore untrusted until the
    /// circuit has checked it. The trust root is checked against the client state's fingerprint.
    /// @param slot The Stellar ledger sequence number this header claims.
    /// @param ledgerHeaderXdr The XDR of the ledger header.
    /// @param envelopesXdr The SCP externalize envelopes for this slot.
    /// @param quorumSetsXdr The quorum set preimages the envelopes reference by hash.
    /// @param trustRootXdr The XDR of the trusted quorum set, checked against the fingerprint.
    /// @param nextEnvelopesXdr The SCP externalize envelopes for slot + 1.
    /// @param nextTxSetXdr The transaction set slot + 1 externalized.
    /// @dev The last two are what authenticate this header's bucketListHash and
    /// txSetResultHash. SCP signs a StellarValue of (txSetHash, closeTime, upgrades) and nothing
    /// else, so those two header fields are covered by no signature. Slot + 1's agreed value names
    /// a transaction set whose previousLedgerHash is this ledger's hash, which is what ties the
    /// whole header to consensus. Without it a prover may substitute either field freely, and a
    /// forged txSetResultHash is enough to prove any packet.
    ///
    /// Slot + 1 reuses this header's quorumSetsXdr: validators pledge the same sets across
    /// adjacent slots, and the fixtures confirm it.
    struct Header {
        uint64 slot;
        bytes ledgerHeaderXdr;
        bytes[] envelopesXdr;
        bytes[] quorumSetsXdr;
        bytes trustRootXdr;
        bytes[] nextEnvelopesXdr;
        bytes nextTxSetXdr;
    }

    /// @notice Proves what a transaction in a ledger produced.
    /// @dev The transaction results are bound to the ledger by the consensus state's
    /// transactionSetResultHash, which the header verification already established.
    /// @param resultPairs Every transaction result in the ledger, in order.
    /// @param resultIndex Which of them is the router invocation.
    /// @param successPreimageXdr The preimage of that invocation's success hash, carrying its events.
    struct StateRootProof {
        bytes[] resultPairs;
        uint32 resultIndex;
        bytes successPreimageXdr;
    }

    /// @notice Defines the supported zk algorithms
    enum SupportedZkAlgorithm {
        Groth16,
        Plonk
    }
}
