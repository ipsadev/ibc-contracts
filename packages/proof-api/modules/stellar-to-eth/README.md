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
3. Waits until the ipsa api holds the SCP messages of the ledger **and its
   successor**: the next-slot binding needs ledger + 1.
4. Assembles and verifies both the packet's ledger and the client's latest ledger
   from the api (all eight steps plus the result-set check), with
   `ipsa-scp-verifier`.
5. Rebuilds the client's trusted consensus state from the verified ledger. The
   client stores only `keccak256(abi.encode(consensusState))`, so the rebuilt
   state is refused unless it hashes to the stored value.
6. Builds the membership witness: the ledger's result pair, the success preimage
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

**`stellar_api_url`** is required: the ipsa api (for example `https://api.ipsa.dev`) that serves Stellar's consensus evidence. An api backed by a stellar-core watcher serves a ledger's SCP messages as it closes, where the history archive publishes them only with its checkpoint of 64 ledgers. The module reads the SCP messages from `/archive/scp/{ledger}`, the header from `/archive/ledger-header/{ledger}`, the next ledger's transaction set from `/archive/tx-set/{ledger}`, and the transaction results from the close metadata of `/ledger/{ledger}`, or from `/archive/results/{ledger}` once Soroban RPC no longer keeps the ledger. Before proving it checks the header against its SCP messages, the results against the header, and the next slot against the ledger's hash, so the api adds no trust. `checkpoint_timeout_seconds` (default 1200) bounds the wait in step 3. The module used to read the history archive directly when no api was set; that path was removed with the move to Protocol 30, because `ipsa-scp-verifier` 3 no longer exports evidence from the archive. `tests/stellar_api.rs` checks, when `STELLAR_LIVE_API_URL` is set, that the api's evidence verifies for a ledger the archive has published, one it has not, and one Soroban RPC no longer keeps.

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

The module checks evidence with `ipsa-scp-verifier` 3.1.0 from crates.io (as `ipsa-scp-exporter`), and the Stellar guests use the same version through the workspace entry. 3.1.0 sits on `stellar-consensus-verifier` 1.1.0, which reads Protocol 30's millisecond close times (CAP-88); 1.0 refused every header since testnet's upgrade at ledger 5540313. Moving the guests onto it recompiled them and moved all three Stellar verifying keys (`ibc-solidity/programs/sp1-stellar-programs/vkeys.json`), so the deployed `SP1ICSStellar`, whose keys are immutable, has to be replaced by one built with the new keys.

## XDR version

Transaction meta and ledger headers are decoded with `stellar-xdr` 30, which knows Protocol 30's `StellarValue` kinds 3 and 4. 28 and earlier fail on every ledger since the upgrade.

## Tests

`tests/witness.rs` replays a saved `getTransaction` response for a real
acknowledgement (`f2755a23…`, ledger 4,902,459) and requires the Rust witness to
reproduce, field for field, the one `fetch-packet-witness.py` produced for it,
given that witness's siblings. It also refuses a path the transaction did not
write, a root from another contract, a sibling that does not fold to the root,
and a list shorter than 64. The fixture predates the incremental tree;
`fetch-packet-witness.py` still reads the old leaf map and cannot record new ones.
