# stellar-light-client-membership

Proves that a packet commitment is, or is not, in the Stellar IBC router's state
at a ledger the client already trusts.

Phase 1's header verification proves a ledger closed. **That says nothing about a
packet.** This is the half that makes `recv_packet`, `acknowledge_packet` and
`timeout_packet` possible.

## The proof path, which does not go through the bucket list

The obvious reading of "prove Stellar state" is a bucket-list membership proof
against `bucketListHash`. That is not how this works, and it is worth knowing
why: Stellar's bucket list hashes whole buckets, not individual entries, so
proving one ledger entry would mean shipping a whole bucket.

The Soroban router sidesteps it. It maintains its own sparse Merkle tree over IBC
paths and **publishes the root as a contract event** on every commit. So the
chain of custody runs through transaction results instead:

```
LedgerHeader.txSetResultHash        established by the header verification
  -> the full set of transaction results, hashed and compared
    -> one result, which must be a successful InvokeHostFunction
      -> that invocation's success preimage, which carries its events
        -> the router's "ibc_root" event
          -> the SMT root
            -> the sibling path for one IBC path
```

Every link is already implemented in `stellar-consensus-verifier`'s
`results::verify_state_root`, which the Cosmos-side `08-wasm` client has been
using for the same purpose. This crate is the EVM-side caller of it.

## What must be pinned, and why

Two values come from the **client state**, never from the prover:

| | Without it |
|---|---|
| `routerContractId` | a prover deploys their own contract, emits `ibc_root` with a root of their choosing, and proves anything |
| `rootEventTopic` | the same attack with a different topic on a contract that legitimately emits other events |

This is the same class of hole as the trust root in
[`../update-client`](../update-client/README.md), and it is closed the same way.
`transactionSetResultHash` is the third anchor: it comes from the consensus state
the contract already stored, so the results cannot be swapped for another
ledger's.

## Proof shape: siblings, not ICS-23

The tree is a fixed-depth-64 sparse Merkle tree, so a proof is exactly 64 sibling
hashes. ICS-23 carries the same siblings inside a protobuf envelope, and the
Cosmos client decodes that envelope only to throw it away and fold the siblings.

The contract reading this output is one we define, so the envelope buys nothing
and costs zkVM cycles to decode. `MembershipProofEntry` therefore takes
`bytes32[] siblings` directly. **The tree is identical** — same `key_index`, same
leaf and inner domain separation, same depth — as `ipsa-core`, the Soroban router
and the Cosmos client. Only the wire envelope differs, and converting an ICS-23
proof into this form is the proof API's job.

An empty `value` means non-membership, which is what a timeout needs.

## Cost

Measured with the executor, against a 16-leaf tree:

| Pairs | Cycles |
|---|---|
| 1 | 156,428 |
| 2 | 283,761 |
| 4 | 536,481 |
| 8 | 1,048,732 |
| 16 | 2,072,683 |

Linear at about **128,000 cycles per pair** over a ~29,000 base.

For comparison, one header update is about 2.69M cycles — roughly twenty-one
packets' worth. **That is the whole argument for batching:** one update plus
sixteen memberships is 4.8M cycles, while sixteen separate update-and-membership
proofs would be 45.6M.

## Testing

```sh
cargo test -p stellar-light-client-membership
cargo run --release -p sp1-stellar-prover --bin membership -- 4
```

13 tests, in `tests/`. The interesting ones are negative: another contract's
event, another topic, results that belong to a different ledger, a tampered
result pair, an out-of-range result index, a short proof, a present key claimed
absent, and a substituted value.

`scenario` builds the XDR by hand — contract events, success preimages, result
pairs and the tree — because `verify_state_root` has no fixtures and, upstream,
no tests at all. It is a feature-gated module rather than a test helper so the
host harness and the tests share one implementation.
