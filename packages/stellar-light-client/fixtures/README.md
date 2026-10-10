# Fixtures

Two pinned ledgers, copied from `ipsa-scp-verifier/tests/fixtures/` so the fork
builds and tests without a second checkout. The published `ipsa-scp-verifier`
crate excludes its fixtures, so they cannot come from the dependency.

Everything here is raw, hex-encoded archive bytes. Nothing is a decoded struct,
which is the point: the verifier decodes them, and a fixture that stored decoded
values would be testing the decoder against itself.

| | Ledger | Envelopes | Notes |
|---|---|---|---|
| `mainnet-63907880.json` | mainnet 63,907,880 | 21 | |
| `mainnet-63907881.json` | mainnet 63,907,881 | 21 | the successor, for step 7 |
| `testnet-4160279.json` | testnet 4,160,279 | 3 | |
| `testnet-4854458.json` | testnet 4,854,458 | 3 | the deployed client's genesis lineage |
| `testnet-4854459.json` | testnet 4,854,459 | 3 | what `sepolia-testnet.json` trusts |
| `testnet-4854948.json` | testnet 4,854,948 | 3 | carries a packet witness |

## Where `bucket_list_hash` comes from, and why it is not circular

`bucket_list_hash` is the field Phase 2's membership proof hangs off: it commits
to all Stellar ledger state, including the IBC router's storage. It therefore
needs a check that does not come from the same decoder that produces it.

Regenerating a fixture with `ipsa-scp-verify --export` would **not** give one —
`to_fixture` writes the value straight out of the verified result, so the fixture
would agree with the decoder by construction.

So the values here were taken from Stellar's own published record instead:

```sh
./verify-against-horizon.py
```

That script fetches each ledger from Horizon, confirms the published
`header_xdr` is **byte-identical** to the fixture's `ledger_header_xdr`, and then
parses the bucket list hash out of it with a **hand-written XDR reader that does
not use `stellar-xdr`**. The parse is self-validating: `ledgerSeq` sits directly
after `bucketListHash` in a `LedgerHeader`, so if the offsets were wrong the
sequence number would not land on Horizon's `sequence`. It does.

Both fixtures pass every check, and the value the circuit commits is the value
Horizon's header decodes to.

Re-run it whenever a fixture is added or changed. It needs network; the Rust
tests do not, because the checked values are pinned in the JSON.

`verified_against` in each file records which endpoint was consulted and when.

## The packet witness

`testnet-4854948.json` has a `packet` section the others do not. It is what the
membership program needs on top of a proven ledger: the ledger proves a
`txSetResultHash`, and the packet witness is the path from that hash to one IBC
packet commitment.

```sh
./fetch-packet-witness.py testnet-4854948.json <transaction hash> <router C...>
```

Run it after `ipsa-scp-verify --export` has written the fixture for the ledger the
transaction landed in. It needs a Soroban RPC endpoint, because the chain of
custody runs through transaction meta and **the history archives do not publish
transaction meta**. That is not a gap in the evidence: every value it fetches is
checked against something the archive already committed to.

| Field | Where it comes from | What proves it |
|---|---|---|
| `result_index` | RPC `applicationOrder` minus one | the result pair at that index carries the transaction hash |
| `success_preimage_xdr` | re-encoded from transaction meta | `sha256` equals the `success` value inside the archived result pair |
| `ibc_root` | the `ibc_root` event in that preimage | the preimage is authenticated above; the event's contract id must be the router |
| `path`, `value` | the contract data the transaction wrote | the router's own leaf map holds `leaf_hash(path, value)` at `key_index(path)` |
| `siblings` | built from that leaf map | folding them from the leaf reproduces the published `ibc_root` |

So the only thing taken on trust from RPC is *where to look*. Every value is
either hashed into something the quorum signed, or discarded.

The preimage is rebuilt with `stellar xdr encode` rather than by hand, for the
same reason the bucket list hash is checked against Horizon: a preimage this
repository serialised itself would be testing the serialiser against itself.
Protocol 23 moved contract events from `soroban_meta.events` to
`operations[].events`, so the script reads both `TransactionMeta` v3 and v4 and
refuses anything else rather than guessing.
