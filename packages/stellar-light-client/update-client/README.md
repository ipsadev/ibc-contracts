# stellar-light-client-update-client

Turns the ABI-encoded inputs an EVM contract holds into the `HeaderInput` that
`ipsa-scp-verifier` verifies, and turns the result back into an ABI-encoded
output a Solidity contract can decode.

This crate is the whole of the Stellar light client's verification logic. The SP1
guest at `ibc-solidity/programs/sp1-programs/stellar-update-client` is a thin
shell around `update_client`: read four byte strings, decode, call, commit.

It mirrors `packages/tendermint-light-client/update-client`, which is upstream's
equivalent for Tendermint. Where the two differ, the difference is Stellar's.

## The types

Defined as Solidity, in
`ibc-solidity/contracts/light-clients/sp1-stellar/msgs/`, and generated into Rust
by `alloy_sol_types::sol!`. Solidity is the source of truth because the committed
output is read by a contract, and a contract cannot decode a bincode blob.

Every `sol!` invocation lives in the same Rust module on purpose. That is how
alloy resolves a Solidity `import` to a sibling generated module rather than
producing a second, incompatible copy of the imported type.

| | Held by | Meaning |
|---|---|---|
| `ClientState` | the contract | network, trust root fingerprint, latest height, frozen flag |
| `ConsensusState` | the contract | a ledger already accepted: hash, close time, bucket list hash |
| `Header` | the prover | the ledger header, its SCP envelopes, the quorum set preimages, the trust root |
| `UpdateClientOutput` | committed | everything above plus the new consensus state and both heights |

## The one thing this must not get wrong

`ipsa-scp-verifier` natively defaults to deriving the trust root from the data
being verified — the quorum set the signers themselves pledge. Natively that is a
weak default. **In a circuit it is fatal:** a prover supplies their own
three-validator quorum set, satisfies every step honestly, and produces a valid
proof of a ledger Stellar never agreed.

So `update_client` never reads a trust root that is not pinned:

1. The client state holds `trustRootFingerprint`, and the contract holds the
   client state across updates.
2. The prover supplies `trustRootXdr` in the header.
3. `update_client` computes `sha256(trustRootXdr)` and **rejects the update
   unless it equals the pinned fingerprint.**
4. Only then is a `QuorumConfig` built from those bytes.

The prover still supplies the bytes; it cannot choose which quorum set they are.

`networkId` is pinned in the client state for the same reason. Without it, a
proof of a testnet ledger verifies identically to a proof of a mainnet one — the
network id is what the envelope signatures are computed over, so it must come
from the contract rather than the header.

## Two other choices that are security-relevant

**The ledger hash is derived, never asserted.** `HeaderInput.recorded_ledger_hash`
is passed as `None`. The verifier will compare the header XDR's hash against a
supplied value when given one, but a value supplied by the prover proves nothing.
Passing `None` means the hash in the output is `sha256(ledgerHeaderXdr)`,
computed inside the circuit.

**Chain continuity is bound only when it is real.** If the proposed slot is
exactly one above the client's latest height, `trusted_previous_hash` is set to
the trusted consensus state's ledger hash, and the circuit rejects a header that
does not chain back to it. For a gap of more than one slot there is no chain to
check, and safety rests on the pinned quorum set having externalized the ledger —
which is the property the eight steps establish. A gap is therefore allowed, and
the client's height only ever moves forward.

## What this does not cover

`verify_header` runs the steps that bind a ledger to its consensus. It does not
run `verify_result_set` or `verify_next_slot_binding`, so `result_pairs_xdr` is
empty here. More importantly, **it says nothing about IBC state**: the header
commits to `bucketListHash`, and getting from there to a packet commitment is a
bucket-list membership proof, which is a separate program.

A contract that has verified one of these proofs knows a Stellar ledger closed.
It does not yet know anything about a packet.

## Failure is a panic, not a flag

`update_client` returns a `Result`, and the guest unwraps it. That is deliberate:
an unprovable ledger should produce no proof at all. Committing `verified: false`
would hand a contract a value it could have assumed without paying for a proof.
