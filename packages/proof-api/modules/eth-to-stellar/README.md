# `proof-api-eth-to-stellar`

A proof-api module that relays IBC v2 packets from an EVM chain's `ICS26Router`
to a Soroban IBC router that holds an SP1 Ethereum light client. It is served by
`programs/stellar-proof-api` beside `stellar_to_eth`.

## What `RelayByTx` does

Given one EVM transaction hash:

1. Reads the receipt and the router's events in it. A `SendPacket` from
   `src_client_id` to `dst_client_id` becomes a Soroban `recv_packet`; a
   `WriteAcknowledgement` for a packet from `dst_client_id` becomes an
   `acknowledge_packet`.
2. Refuses, before anything slow or paid, a receive the Soroban router already
   holds a receipt for, and an acknowledgement whose packet commitment is gone.
   Both are read straight from the router's persistent storage, where each is
   keyed by its IBC path.
3. Finds the light client behind `dst_client_id` on the router and its latest
   slot, stored consensus hash and client-state commitment.
4. **Rebuilds the trusted consensus state from the beacon chain**: the state root,
   timestamp and current committee from the bootstrap at that slot, and — when
   the stored hash says the client knows it — the next committee from the period's
   best light-client update. The Soroban client stores only a hash, so the rebuilt
   state is refused unless it hashes to it. The client state is rebuilt the same
   way and checked against the pinned commitment.
5. Waits until a finality update with at least two thirds of the committee signing
   finalizes the transaction's block.
6. Builds the chain of headers from the trusted slot to that update, the way
   upstream's `eth-to-cosmos` does: one header per sync-committee period crossed,
   each the new period's best update verified against the committee the previous
   state knew as *next*, then the finality update. Each step's resulting state is
   computed on the host with upstream's `update_consensus_state`.
7. Fetches the storage proof for the packet or acknowledgement commitment at the
   last header's finalized block, retrying `eth_getProof` through the
   intermittent `distance to target block exceeds maximum proof window` refusals,
   and requires the stored value to be the one the packet implies.
8. Executes every header and the membership in the guests and requires each
   committed output to equal what the host computed — the hashing is the guests'
   own `wire.rs`, compiled into `sp1-ethereum-prover` — then proves them as
   Groth16 on the Succinct network.
9. Returns the Soroban calls, in order, as the gateway encodes them for a Stellar
   destination — a big-endian `u32` count, then each `HostFunction`'s XDR:
   `update_client` per header, then `recv_packet` or `acknowledge_packet`. The
   caller signs and submits them.

## What `CreateClient` does

It takes one parameter, `client_type`, and builds a genesis at the beacon chain's
latest finalized checkpoint: the state root, timestamp and current committee from
that checkpoint's bootstrap, and the **next committee** from the period's best
light-client update. It derives both verifying keys from the configured ELFs and
returns a single Soroban `create_client` on the router. The client state is
the two verifying keys followed by the client-state commitment. The consensus
state is the consensus hash followed by the timestamp. The caller signs and
submits it.

## Crossing sync-committee periods

A client learns the next committee only from an update whose attested period is
its stored period and whose finalized slot is newer than the one it trusts. The
beacon API's only committee-carrying update for a period is its best update, which
finalizes at the period's start — older than any state a client created inside
that period holds. So a client must be **created with its next committee
already in its genesis state**, as upstream does and as `CreateClient` here does.
A client created without it can never cross, and `RelayByTx` says so rather than
trying.

From then on, each period's best update carries the committee after it, so a
client can cross any number of periods: step 6 adds one header per period. This
ran on Sepolia on 2026-09-28, crossing one period inside a relayed receive. On
2026-10-03, `catch-up` walked a client four periods (1372 to 1376) in the guest
without proving.

## Checking that a client can catch up

A client left idle for several periods is brought forward by step 6, one header
per missed period. `catch-up` runs that chain on its own, without a packet and
without proving:

```sh
cargo run --release -p proof-api-eth-to-stellar --bin catch-up
```

It reads the client's latest slot, consensus hash and client-state commitment
from the Soroban router, rebuilds and checks the trusted state as step 4 does,
builds the headers to the latest finalized slot with the module's own
`Beacon::steps`, and executes each in the update-client guest, requiring its
output to equal the host's. Nothing is sent and nothing is paid for.

| Variable | Default |
|---|---|
| `BEACON_API_URL` | `https://ethereum-sepolia-beacon-api.publicnode.com` |
| `SOROBAN_RPC_URL` | `https://soroban-testnet.stellar.org` |
| `SOROBAN_ROUTER` | the round 4 router, `CAWG2HO4…` |
| `STELLAR_EVM_CLIENT_ID` | `sepolia-0` |
| `EVM_ICS26_ROUTER` | the Sepolia `ICS26Router`, `0x3f09B9D0…` |
| `EVM_CHAIN_ID` | `11155111` |
| `SP1_ETHEREUM_ELF` | the update-client ELF under `programs/sp1-ethereum-programs` |

The verifying key it prints must match `programs/sp1-ethereum-programs/vkeys.json`.

## Configuration

```json
{
  "name": "eth_to_stellar",
  "src_chain": "11155111",
  "dst_chain": "stellar-testnet",
  "config": {
    "beacon_api_url": "https://…",
    "eth_rpc_url": "https://…",
    "ics26_address": "0x…",
    "soroban_rpc_url": "https://soroban-testnet.stellar.org",
    "soroban_router": "C…",
    "sp1_programs": {
      "update_client": "…/sp1-ethereum-update-client",
      "membership": "…/sp1-ethereum-membership"
    },
    "prover": "network"
  }
}
```

`ibc_commitment_slot` defaults to `ICS26Router`'s storage slot, and
`finality_timeout_seconds` (default 1800) bounds step 5. `prover` is `network`
or `execute`; `execute` runs everything up to proving and returns an empty
transaction.

## Not built yet

- More than one packet per transaction or transaction per request
- Timeouts: `timeout_tx_ids` are refused
- `UpdateClient` on its own: clients are updated only inside a relay, and
  `catch-up` checks an update chain without sending it
- Reusing a proof: a relay that is retried after a failed delivery proves again
