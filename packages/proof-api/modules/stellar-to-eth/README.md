# `proof-api-stellar-to-eth`

A proof-api module that relays IBC v2 packets from Stellar (Soroban) to an EVM
chain whose `ICS26Router` holds an `SP1ICSStellar` light client. It is served by
`programs/stellar-proof-api`, which registers it with upstream's
`ProofApiBuilder`; upstream's own `programs/proof-api` is left untouched.

## What `RelayByTx` does

Given one Stellar transaction hash:

1. Reads the transaction from Soroban RPC and the router's events in it. A
   `send_packet` from `src_client_id` to `dst_client_id` becomes a `recvPacket`;
   a `recv_packet` of a packet from `dst_client_id`, with its `write_ack`, becomes
   an `ackPacket`.
2. Refuses, before anything slow or paid, a receive the EVM router already holds
   a receipt for, and an acknowledgement whose packet commitment is already gone.
   A receipt can also mean a **sequence collision**: when `migrateClient` points
   an existing client id at a new Soroban router, that router's sequences start
   again at 1 under the same id. Then reads the EVM client behind `dst_client_id`
   and refuses unless it pins this module's Soroban router.
3. Waits for the history archive to publish the checkpoint holding the ledger
   **and its successor** — the next-slot binding needs ledger + 1.
4. Exports and verifies both the packet's ledger and the client's latest ledger
   through `ipsa-scp-verifier`'s `api::export` (all eight steps plus the
   result-set check).
5. Rebuilds the client's trusted consensus state from the archived ledger. The
   client stores only `keccak256(abi.encode(consensusState))`, so the rebuilt
   state is refused unless it hashes to the stored value.
6. Builds the membership witness: the archived result pair, the success preimage
   rebuilt from the transaction meta, the value the transaction wrote at the
   packet's path, and the path's 64 sparse Merkle siblings, folded to the
   published `ibc_root`. The committed value must equal what the EVM router
   will recompute from the packet.

   The siblings come from `ipsa-gateway` (`QueryTreeSiblings`). The router keeps
   its tree as separate entries per leaf and branch point, so a transaction
   writes only its own path and the siblings are not in its meta. The gateway
   mirrors the tree and rebuilds it as it was at the transaction's ledger. The
   gateway is not trusted for this: siblings that do not fold the written value
   to the root the transaction published are refused.
7. Runs update-client and membership natively, then proves both as Groth16 on the
   Succinct network.
8. Returns `ICS26Router.multicall(updateClient, recvPacket | ackPacket)` calldata
   and the router address. Submitting it is the caller's job.

## Configuration

```json
{
  "name": "stellar_to_eth",
  "src_chain": "stellar-testnet",
  "dst_chain": "11155111",
  "config": {
    "soroban_rpc_url": "https://soroban-testnet.stellar.org",
    "stellar_network": "testnet",
    "soroban_router": "C…",
    "eth_rpc_url": "https://…",
    "ics26_address": "0x…",
    "sp1_programs": {
      "update_client": "…/sp1-stellar-update-client",
      "membership": "…/sp1-stellar-membership"
    },
    "prover": "network",
    "gateway_grpc_address": "http://127.0.0.1:50052"
  }
}
```

**`gateway_grpc_address`** is required to relay packets: without it every
request fails before proving, with a message that says why.

`archive_urls` overrides the network's default archive, and
`checkpoint_timeout_seconds` (default 1200) bounds the wait in step 3.

**`prover`** is `network` (default) or `execute`. `execute` is a free dry run:
it does every step up to proving, executes both guests on the CPU, logs the cycle
counts, and returns an **empty** `tx`. The result goes to the log rather than to
the caller because upstream's core replaces every module error with a generic
`Internal` status.

The network prover reads `NETWORK_PRIVATE_KEY` from the environment — the
Succinct requester account whose deposited PROVE pays for the proofs.

## Not built yet

- More than one packet in a transaction, or more than one transaction per request
- Relaying a ledger at or behind the client's latest height
- Timeouts (`timeout_tx_ids`), `CreateClient`, `UpdateClient`
- The combined uc-and-membership program, which would halve the proofs

## The `ipsa-scp-verifier` dependency

The exporter comes from the sibling checkout, as `ipsa-scp-exporter`, because
`api::export` is not yet published. Only this module uses it. The Stellar guests
keep `ipsa-scp-verifier` 1.0.0 from crates.io: repointing the workspace entry
would recompile them and move every verifying key. The path dependency is
replaced by the published version before the corridor is committed.

## XDR version

Transaction meta is decoded with `stellar-xdr` 28, and the exporter uses 28 too.
Protocol 28 ledgers carry transaction sets that 27 cannot decode, and the
exporter decodes every set in a checkpoint to find each one's byte span, so a
single such ledger made its whole checkpoint unexportable. The guests do not
decode sets and are unaffected.

## Tests

`tests/witness.rs` replays a saved `getTransaction` response for a real
acknowledgement (`f2755a23…`, ledger 4,902,459) and requires the Rust witness to
reproduce, field for field, the one `fetch-packet-witness.py` produced for it,
given that witness's siblings. It also refuses a path the transaction did not
write, a root from another contract, a sibling that does not fold to the root,
and a list shorter than 64. The fixture predates the incremental tree;
`fetch-packet-witness.py` still reads the old leaf map and cannot record new ones.
