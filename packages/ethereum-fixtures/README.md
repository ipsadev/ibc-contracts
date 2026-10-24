# Ethereum fixtures

Written by `cargo run -p sp1-ethereum-prover --bin fixture`, and **not committed**.

The Stellar fixtures are pinned because a closed ledger is closed forever. These are not: a
finality update is only useful while the sync committee that signed it is still inside the
client's trusted window, and the beacon API serves whatever is current. Committing one would
produce a fixture that passes today, fails next week, and looks like a regression in the guest.

So the guest is exercised against freshly fetched data, and what gets pinned is the verifying
key — which does not depend on the chain's state.

```sh
cargo run -p sp1-ethereum-prover --bin fixture
BEACON_API=https://your-node cargo run -p sp1-ethereum-prover --bin fixture
```

**Expected:** `signers` near 512 of 512, then four files here.

The run refuses anything below two thirds. A finality update fetched mid-slot can carry almost
no signatures — this endpoint served 15 of 512 one minute and 509 the next — and such a fixture
is not wrong, just useless: the guest would reject it and the time would go on debugging the
guest instead of re-running this.

`GENESIS_SLOT` is patched in when a node omits it from `/config/spec`, which several do. It is 0
on every network that has ever run.
