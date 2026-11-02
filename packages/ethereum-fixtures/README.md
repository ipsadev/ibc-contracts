# Ethereum fixtures

Written by `cargo run -p sp1-ethereum-prover --bin fixture`. The finality data it fetches (`*.json` and `timestamp.txt`) is **not committed**; only `public-values.hex` and `membership-value.bin` are, because `sp1-ethereum-prover/tests/wire.rs` decodes them offline to check the wire format the Soroban client reads.

The Stellar fixtures are pinned because a closed ledger is closed forever. These are not: a finality update is only useful while the sync committee that signed it is still inside the client's trusted window, and the beacon API serves whatever is current. Committing one would produce a fixture that passes today, fails next week, and looks like a regression in the guest.

So the guest is exercised against freshly fetched data, and what gets pinned is the verifying key, which does not depend on the chain's state. The deterministic, committed Sepolia set the prover's own tests use is in `packages/sp1-ethereum-prover/tests/fixtures/sepolia-11306528`.

```sh
cargo run -p sp1-ethereum-prover --bin fixture
BEACON_API=https://ethereum-sepolia-beacon-api.publicnode.com EXECUTION_RPC=https://ethereum-sepolia-rpc.publicnode.com ETHEREUM_CHAIN_ID=11155111 cargo run -p sp1-ethereum-prover --bin fixture
```

With no variables set it fetches from Ethereum mainnet (chain id 1); the second line fetches Sepolia, the chain the deployed corridor runs on. The chain id is not derivable from the beacon API and is part of what the client pins, so set it whenever you change the endpoints.

**Expected:** `signers` near 512 of 512, then the files here.

The run refuses anything below two thirds. A finality update fetched mid-slot can carry almost no signatures (this endpoint served 15 of 512 one minute and 509 the next), and such a fixture is not wrong, just useless: the guest would reject it and the time would go on debugging the guest instead of re-running this.

`GENESIS_SLOT` is patched in when a node omits it from `/config/spec`, which several do. It is 0 on every network that has ever run.
