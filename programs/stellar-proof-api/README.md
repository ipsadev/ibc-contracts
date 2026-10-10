# stellar-proof-api

The proof API for ipsa's Stellar corridors. It serves `RelayByTx` on port 3100 with
three modules:

- `stellar_to_eth` proves Stellar router state for the `SP1ICSStellar` client on
  Ethereum, and builds that client's creation transaction.
- `eth_to_stellar` proves Ethereum beacon headers and storage for the
  sp1-ethereum light client on Stellar, and builds that client's state.
- `forward` hands Cosmos pairs to `ipsa-gateway`.

## Configuration

`start --config <file>` reads a JSON config such as `testnet.json`. When
`PROOF_API_CONFIG_JSON` is set and not blank, its contents are used instead and
the file is never opened, so the service can run where no file can be mounted.

SP1 settings come from the environment: `SP1_PROVER=network`,
`SP1_PROOF_MODE=groth16`, and `NETWORK_PRIVATE_KEY`, the Succinct prover
network key that pays for proofs in PROVE.

Program paths in the config are relative to the working directory.

## Image

```sh
programs/stellar-proof-api/build-image.sh            # builds and loads locally
programs/stellar-proof-api/build-image.sh --push     # builds and pushes
```

`IMAGE` (default `amandagonsalvesx/ipsa-proof-api`), `TAG` (default the short
commit), `PLATFORM` (default `linux/arm64`) and `CONFIG` (default `testnet.json`)
override the defaults.

The SP1 ELFs and the `SP1ICSStellar` artifact are not built in the image. The
script stages exactly the files the config names, from this working copy, and
passes them as a separate build context, because they fix the program
verification keys every new client is created with: an image must carry the same
ELFs as the clients it will prove for. The image keeps them under `/app` at the
same relative paths, and starts from `/app`.
