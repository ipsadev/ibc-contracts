# sp1-stellar-prover

Host-side harness for the Stellar SP1 light client: builds a witness from a
pinned archive fixture, runs the guest, and checks the result against the native
verifier.

## Build the guest first

```sh
cd ibc-solidity/programs/sp1-stellar-programs/update-client
cargo prove build --locked
```

The guest is its own Cargo workspace with its own `[patch.crates-io]`, so the
ELF lands under that directory's own
`target/elf-compilation/riscv64im-succinct-zkvm-elf/release/`. Both binaries here
read it from there, or from `SP1_STELLAR_ELF`.

It is separate on purpose: making it a member of upstream's `sp1-programs`
workspace re-resolves that shared lockfile and changes upstream's own compiled
programs and their verifying keys, patches or no patches.

## `differential` — the executor, and the check that matters

```sh
cargo run --release -p sp1-stellar-prover --bin differential
cargo run --release -p sp1-stellar-prover --bin differential -- \
  packages/stellar-light-client/fixtures/testnet-4160279.json
```

Runs the **executor only**, which takes seconds rather than minutes, then
compares the committed output against three independent sources:

1. `verify_header` run natively on the same input, field by field
2. the client state echoed back, so a circuit that quietly used different trust
   parameters is caught
3. the ledger hash, close time, bucket list hash and result hash **recorded in
   the archive fixture**, which neither the circuit nor the native verifier
   produced

It prints the cycle count and exits non-zero on any mismatch.

Run this before ever paying for a proof. It catches every encoding bug, and an
encoding bug is what a serialisation change looks like from the outside.

## `prove` — a real proof

```sh
cargo run --release -p sp1-stellar-prover --bin prove
SP1_PROOF_MODE=compressed cargo run --release -p sp1-stellar-prover --bin prove
```

Sets up a proving key, proves, verifies against the verifying key, decodes the
public values, and asserts the same fields. `SP1_PROOF_MODE` accepts `core`
(default), `compressed`, `groth16` and `plonk`. Groth16 and Plonk are what an EVM
verifier needs and want considerably more memory.

It prints the vkey, which is the value the on-chain contract has to pin.

## What the numbers should be

| Fixture | Envelopes | Cycles | Checks |
|---|---|---|---|
| `mainnet-63907880` | 21 | 2,690,852 | 14 |
| `testnet-4160279` | 3 | 412,612 | 14 |

Around 17.4M on mainnet means the ed25519 precompile is not applying.

A core-mode proof of the mainnet ledger takes about 67 seconds on CPU and
produces vkey
`0x00a7a491a54013937c571cea498e683c3b09382ac04e9cba0cab700f8f7106b4`.

## The fixture's trust root is a test choice

A fixture carries the quorum sets the envelopes reference; it does not carry a
separately pinned trust root, because in production that is chosen by whoever
deploys the contract. The harness uses `quorum_sets_xdr[0]` and derives the
fingerprint from it, which mirrors what the native tool does by default.

That is fine for a differential test — both sides are given the same trust root,
so a disagreement is a real disagreement. **It is not a model for deployment.**
Choosing and rotating the trust root is a governance question, not a harness one.

The trusted consensus state is likewise synthesised: the trusted height is set to
one below the fixture's slot and the trusted ledger hash to the header's own
previous-ledger hash, so the adjacency binding is exercised rather than skipped.
