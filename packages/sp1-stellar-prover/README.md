# sp1-stellar-prover

Host-side harness for the Stellar SP1 light client: builds a witness from a pinned archive fixture, runs the guests, checks the result against the native verifier, and keeps the pinned verifying keys and the Solidity fixtures in step with the guests.

## Build the guests first

```sh
for program in update-client membership uc-and-membership; do
  (cd "ibc-solidity/programs/sp1-stellar-programs/$program" && cargo prove build --docker --tag v6.1.0 --workspace-directory ../../../.. --output-directory target/elf-compilation/riscv64im-succinct-zkvm-elf/release --locked)
done
for program in update-client membership; do
  (cd "programs/sp1-ethereum-programs/$program" && cargo prove build --docker --tag v6.1.0 --workspace-directory ../../.. --output-directory target/elf-compilation/riscv64im-succinct-zkvm-elf/release --locked)
done
```

`--docker` compiles inside SP1's `ghcr.io/succinctlabs/sp1:v6.1.0` image, so every machine gets the same ELF and therefore the same verifying key. A host build embeds the host's paths and toolchain, and a macOS build and CI's Linux build gave different keys for all five programs. `--workspace-directory` mounts the repository root, because the guests depend on crates outside their own directories. A Docker build writes its ELF under `target/elf-compilation/docker/`, so `--output-directory` copies it to the usual path, where the binaries here and the proof API image read it. Build with Docker before pinning keys or making proofs that will be verified on chain.

Each guest is its own Cargo workspace with its own `[patch.crates-io]`, so its ELF lands under that directory's own `target/elf-compilation/riscv64im-succinct-zkvm-elf/release/`. The binaries here read it from there, or from `SP1_STELLAR_ELF` (update-client) and `SP1_STELLAR_MEMBERSHIP_ELF` (membership).

The Stellar guests are kept out of upstream's `sp1-programs` workspace on purpose: making them members re-resolves that shared lockfile and changes upstream's own compiled programs and their verifying keys, patches or no patches. The `upstream-untouched` job in `.github/workflows/stellar.yml` enforces this.

## `vkeys`: the pinned verifying keys

```sh
cargo run --release -p sp1-stellar-prover --bin vkeys
cargo run --release -p sp1-stellar-prover --bin vkeys -- --write
```

Computes the verifying key of every built guest, Stellar and Ethereum, and compares it with `ibc-solidity/programs/sp1-stellar-programs/vkeys.json` and `programs/sp1-ethereum-programs/vkeys.json`. It exits non-zero if a key moved. `--write` re-pins them after a deliberate guest change. A moved key means every deployed client that pins the old key must be redeployed or migrated, so a re-pin is never routine.

## `differential`: the executor, and the check that matters

```sh
cargo run --release -p sp1-stellar-prover --bin differential
cargo run --release -p sp1-stellar-prover --bin differential -- packages/stellar-light-client/fixtures/testnet-4160279.json
```

Runs the **executor only**, which takes seconds rather than minutes, then compares the committed output against three independent sources:

1. `verify_header` run natively on the same input, field by field
2. the client state echoed back, so a circuit that quietly used different trust parameters is caught
3. the ledger hash, close time, bucket list hash and result hash **recorded in the archive fixture**, which neither the circuit nor the native verifier produced

It prints the cycle count and exits non-zero on any mismatch. Run this before ever paying for a proof: it catches every encoding bug, and an encoding bug is what a serialisation change looks like from the outside.

## `negative`: everything the native verifier rejects, the circuit rejects

```sh
cargo run --release -p sp1-stellar-prover --bin negative
cargo run --release -p sp1-stellar-prover --bin negative -- packages/stellar-light-client/fixtures/testnet-4160279.json
```

Applies one mutation per case to a valid witness and requires both the native verifier and the guest to refuse it.

## `membership`: the packet proof

```sh
cargo run --release -p sp1-stellar-prover --bin membership
```

Executes the membership guest against a router state proof and compares the proven pairs with the native check.

## `prove`: a real proof

```sh
cargo run --release -p sp1-stellar-prover --bin prove
SP1_PROOF_MODE=compressed cargo run --release -p sp1-stellar-prover --bin prove
```

Sets up a proving key, proves, verifies against the verifying key, decodes the public values, and asserts the same fields. `SP1_PROOF_MODE` accepts `core` (default), `compressed`, `groth16` and `plonk`; Groth16 and Plonk are what an EVM verifier needs and want considerably more memory. `SP1_PROVER=network` sends the request to the Succinct network instead. It prints the verifying key, which must equal the `update-client` entry in `vkeys.json`.

## Regenerating what the Solidity side reads

```sh
cargo run --release -p sp1-stellar-prover --bin fixture
cargo run --release -p sp1-stellar-prover --bin genesis
cargo run --release -p sp1-stellar-prover --bin packet
```

- `fixture` writes `ibc-solidity/test/sp1-stellar/fixtures/stellar.json` from the mainnet fixture.
- `genesis` writes `ibc-solidity/scripts/deployments/stellar/genesis.json` from a fixture (the mainnet one by default), the Soroban router in `ROUTER_CONTRACT_ID`, and the pinned verifying keys. The committed file is the genesis of the Protocol 30 client on Sepolia, testnet ledger 5541000 against router `CBA7QRXNLV2VGXNB7JALC4ZRPD4Q3SFAJRXFS3NTUJBFLLIDT6AEECSS`, and CI regenerates it with exactly that fixture and router.
- `packet` proves a real testnet packet against `scripts/deployments/stellar/sepolia-testnet.json` and exports the update and membership proofs the Solidity proof tests replay.

The `solidity-client` job fails if `fixture` or `genesis` would change a committed file.

## What the numbers should be

| Fixture | Envelopes | Cycles | Checks |
|---|---|---|---|
| `mainnet-63907880` | 21 | 2,690,852 | 14 |
| `testnet-4160279` | 3 | 412,612 | 14 |

Around 17.4M on mainnet means the ed25519 precompile is not applying. A core-mode proof of the mainnet ledger takes about 67 seconds on CPU.

## The fixture's trust root is a test choice

A fixture carries the quorum sets the envelopes reference; it does not carry a separately pinned trust root, because in production that is chosen by whoever deploys the contract. The harness uses `quorum_sets_xdr[0]` and derives the fingerprint from it, which mirrors what the native tool does by default.

That is fine for a differential test: both sides are given the same trust root, so a disagreement is a real disagreement. **It is not a model for deployment.** Choosing and rotating the trust root is a governance question, not a harness one.

The trusted consensus state is likewise synthesised: the trusted height is set to one below the fixture's slot and the trusted ledger hash to the header's own previous-ledger hash, so the adjacency binding is exercised rather than skipped.
