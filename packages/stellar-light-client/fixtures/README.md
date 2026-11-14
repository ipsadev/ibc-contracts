# Fixtures

Pinned Stellar ledgers, so the fork builds and tests without network access or a second checkout. The published `stellar-consensus-verifier` crate excludes its own fixtures, so they cannot come from the dependency.

Everything here is raw, hex-encoded archive bytes. Nothing is a decoded struct, which is the point: the verifier decodes them, and a fixture that stored decoded values would be testing the decoder against itself.

| | Ledger | Envelopes | Used by |
|---|---|---|---|
| `mainnet-63907880.json` | mainnet 63,907,880 | 21 | the update-client tests and every `sp1-stellar-prover` bin by default; the only nested, 21-validator quorum |
| `mainnet-63907881.json` | mainnet 63,907,881 | 21 | the successor ledger, for the result-set tests in `membership/tests/result_set.rs` |
| `testnet-4160279.json` | testnet 4,160,279 | 3 | the update-client tests and the `differential` and `negative` CI runs, for a flat quorum |
| `testnet-4854458.json` | testnet 4,854,458 | 3 | the genesis lineage of the deployed testnet client, kept as a record |
| `testnet-4854459.json` | testnet 4,854,459 | 3 | the ledger `scripts/deployments/stellar/sepolia-testnet.json` trusts, kept as a record |
| `testnet-4854948.json` | testnet 4,854,948 | 3 | carries a packet witness; the `packet` bin proves it for the Solidity proof tests |
| `testnet-4902459.json` | testnet 4,902,459 | 3 | carries a packet witness; the proof API's `stellar-to-eth/tests/witness.rs` |

The two mainnet fixtures are the large ones, about 0.7 and 0.9 MB. They stay because they are the only coverage of a nested quorum with real mainnet signers.

## Where the header hashes come from, and why it is not circular

The membership proof hangs off `tx_set_result_hash`: the update proof commits it from the verified ledger header, and the membership proof walks from it to the router's `ibc_root` event. `bucket_list_hash`, which commits to all ledger state, is pinned alongside it. Both need a check that does not come from the same decoder that produces them.

Regenerating a fixture with `ipsa-scp-verify --export` would **not** give one: `to_fixture` writes the values straight out of the verified result, so the fixture would agree with the decoder by construction.

So the values here were taken from Stellar's own published record instead:

```sh
./verify-against-horizon.py
```

That script fetches each ledger from Horizon, confirms the published `header_xdr` is **byte-identical** to the fixture's `ledger_header_xdr`, and then parses the result hash and the bucket list hash out of it with a **hand-written XDR reader that does not use `stellar-xdr`**. The parse is self-validating: `ledgerSeq` sits directly after `bucketListHash` in a `LedgerHeader`, so if the offsets were wrong the sequence number would not land on Horizon's `sequence`.

Re-run it whenever a fixture is added or changed. It needs network; the Rust tests do not, because the checked values are pinned in the JSON. `verified_against`, where present, records which endpoint was consulted and when.

## The packet witness

`testnet-4854948.json` and `testnet-4902459.json` have a `packet` section the others do not. It is what the membership program needs on top of a proven ledger: the ledger proves a `txSetResultHash`, and the packet witness is the path from that hash to one IBC packet commitment.

```sh
./fetch-packet-witness.py testnet-4854948.json <transaction hash> <router C...>
```

Run it after `ipsa-scp-verify --export` has written the fixture for the ledger the transaction landed in. It needs a Soroban RPC endpoint, because the chain of custody runs through transaction meta and **the history archives do not publish transaction meta**. That is not a gap in the evidence: every value it fetches is checked against something the archive already committed to.

| Field | Where it comes from | What proves it |
|---|---|---|
| `result_index` | RPC `applicationOrder` minus one | the result pair at that index carries the transaction hash |
| `success_preimage_xdr` | re-encoded from transaction meta | `sha256` equals the `success` value inside the archived result pair |
| `ibc_root` | the `ibc_root` event in that preimage | the preimage is authenticated above; the event's contract id must be the router |
| `path`, `value` | the contract data the transaction wrote | the router's own leaf map holds `leaf_hash(path, value)` at `key_index(path)` |
| `siblings` | built from that leaf map | folding them from the leaf reproduces the published `ibc_root` |

So the only thing taken on trust from RPC is *where to look*. Every value is either hashed into something the quorum signed, or discarded.

The preimage is rebuilt with `stellar xdr encode` rather than by hand, for the same reason the header hashes are checked against Horizon: a preimage this repository serialised itself would be testing the serialiser against itself. Protocol 23 moved contract events from `soroban_meta.events` to `operations[].events`, so the script reads both `TransactionMeta` v3 and v4 and refuses anything else rather than guessing.
