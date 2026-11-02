# stellar-proof-api

The proof API for ipsa's Stellar corridors. It serves `RelayByTx` on port 3100 with three modules:

- `stellar_to_eth` proves Stellar router state for the `SP1ICSStellar` client on Ethereum, and builds that client's creation transaction.
- `eth_to_stellar` proves Ethereum beacon headers and storage for the sp1-ethereum light client on Stellar, and builds that client's state.
- `forward` hands Cosmos pairs to `ipsa-gateway`.

## Configuration

`start --config <file>` reads a JSON config such as `testnet.json`. When `PROOF_API_CONFIG_JSON` is set and not blank, its contents are used instead and the file is never opened, so the service can run where no file can be mounted.

SP1 settings come from the environment: `SP1_PROVER=network`, `SP1_PROOF_MODE=groth16`, and `NETWORK_PRIVATE_KEY`, the Succinct prover network key that pays for proofs in PROVE.

Program paths in the config are relative to the working directory.

## Proof history

When `PROOF_HISTORY_DB_URL` is set, every proof the modules request is recorded in a `proof_requests` table, created on startup (`packages/proof-api/history`). One row per proof:

- **What it proved:** the module, the chain pair, the kind (`recv`, `ack`, or `relay` for the gateway's ICS-23 proofs), the transaction, the packet sequences, the step within the relay (each header of an update chain, then the membership), the program and its verifying key, the trusted and target heights, the public values and the proof bytes.
- **What it cost:** the Succinct request id and transaction, the fulfiller, cycles, PGUs used and their limit, the PGU price (`gas_price`) and the request's `base_fee`, and the request and fulfilment times. A request's cost in PROVE is `base_fee + gas_used × gas_price`, in 18-decimal units.
- **Failures** are recorded too, with the error.
- **Retries** of a proof that failed or was not ready share its row: the relayer asks again every few seconds, and each attempt updates the status, the error and `last_attempt_at` and adds one to `attempts`. A later success takes over the failed row of the same step, with its program, proof and network request, so a proof keeps one row across retries. A gateway answer of `UNAVAILABLE`, evidence that is not published yet, is recorded as `waiting` rather than `failed`. A proof that succeeded is never overwritten; asking for it again records a new row.

The connection requires TLS and refuses a database url with `sslmode=disable`. The server certificate is verified against the web roots and Amazon's RDS bundle, and connecting gives up after 5 seconds. Recording never fails or stalls a relay: an unreachable database, or a write that takes longer than 10 seconds, is logged and the proof is returned as usual. Without the variable nothing is recorded.

`packages/proof-api/history/certificates/rds-global-bundle.pem` is Amazon's public RDS global CA bundle, downloaded from https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem and embedded in the binary. To refresh it, download that URL over the file (`curl -fsSL https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem -o packages/proof-api/history/certificates/rds-global-bundle.pem`) and rebuild.

### Backfilling from the Succinct network

`proof-history-backfill` fills the table with the requests made before recording existed:

- `proof-history-backfill list` prints every request of the `NETWORK_PRIVATE_KEY` requester as JSON lines.
- `proof-history-backfill insert` reads every request named in `PROOF_BACKFILL_JSON` (`[{"r": request id, "m": module, "s": source chain, "d": destination chain, "k": kind, "t": transaction, "q": [sequences], "n": step, "p": program}]`) from the network, with its record and, while the network still serves it, its proof, and writes it with that packet context. A named request that is not on the network fails the run; a request already in the table is skipped, so running it twice is safe. Requests nobody named are left out: only proofs tied to a transfer the relayer tracked can be shown with it.
- `proof-history-backfill proof <request id>` prints a fulfilled request's proof.

The attributions come from the proof API's own logs: each `found the packet to relay` line is followed by the `Created request` lines of that relay's proofs. When two relays run at once their requests alternate, and the proof bytes, found in the transaction that delivered them, show which is which.

## Image

```sh
programs/stellar-proof-api/build-image.sh                                        # builds and loads ipsa-proof-api locally
IMAGE=<registry>/<repository> programs/stellar-proof-api/build-image.sh --push   # builds and pushes
```

`IMAGE` (default `ipsa-proof-api`, a local name; set it to a registry repository before pushing), `TAG` (default the short commit), `PLATFORM` (default `linux/arm64`) and `CONFIG` (default `testnet.json`) override the defaults.

The SP1 ELFs and the `SP1ICSStellar` artifact are not built in the image. The script stages exactly the files the config names, from this working copy, and passes them as a separate build context, because they fix the program verification keys every new client is created with: an image must carry the same ELFs as the clients it will prove for. The image keeps them under `/app` at the same relative paths, and starts from `/app`.

The image carries no config. Its command reads `/app/proof-api.json`, which it does not contain, so a container needs either `PROOF_API_CONFIG_JSON` set to the config's contents or a config mounted at `/app/proof-api.json` (for example `-v "$PWD/programs/stellar-proof-api/testnet.json:/app/proof-api.json:ro"`).
