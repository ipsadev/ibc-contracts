#!/usr/bin/env python3
"""Adds the packet witness to a verified ledger fixture.

The SCP fixture proves a ledger. This adds what the membership program needs on
top of it: which transaction in that ledger published an ibc_root, the preimage
that hashes to the result the ledger committed to, and a sparse merkle proof of
one packet commitment under that root.

Run it after ipsa-scp-verify --export has written the fixture for the ledger the
transaction landed in.

  ./fetch-packet-witness.py testnet-4854948.json <transaction hash> <router C...>
  ./fetch-packet-witness.py testnet-N.json <recv transaction> <router C...> <ack path hex>

Nothing here is trusted. Every value is checked against something the fixture
already carries: the preimage against the archived result pair, the leaf against
the router's own stored leaf map, and the folded root against the event.
"""

import base64
import hashlib
import json
import struct
import subprocess
import sys
import urllib.request

RPC = {
    "testnet": "https://soroban-testnet.stellar.org",
    "mainnet": "https://mainnet.sorobanrpc.com",
}

TREE_DEPTH = 64
EMPTY = b"\x00" * 32
BASE32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"


def fail(message):
    print(f"FAILED: {message}", file=sys.stderr)
    sys.exit(1)


def rpc(endpoint, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
    request = urllib.request.Request(
        endpoint,
        data=body.encode(),
        headers={"Content-Type": "application/json", "User-Agent": "ipsa-fixtures/1"},
    )
    answer = json.load(urllib.request.urlopen(request, timeout=60))

    if "error" in answer:
        fail(f"{method}: {answer['error']}")

    return answer["result"]


def xdr(action, kind, payload):
    process = subprocess.run(
        ["stellar", "xdr", action, "--type", kind, "--output",
         "json" if action == "decode" else "single-base64",
         *(["--input", "single-base64"] if action == "decode" else [])],
        input=payload,
        capture_output=True,
        text=True,
    )

    if process.returncode != 0:
        fail(f"stellar xdr {action} {kind}: {process.stderr.strip()}")

    return process.stdout.strip()


def decode_contract_strkey(text):
    if len(text) != 56 or not text.startswith("C"):
        fail(f"{text} is not a contract strkey")

    raw = base64.b32decode(text + "=" * ((8 - len(text) % 8) % 8))

    if raw[0] != 2 << 3:
        fail(f"{text} has version byte {raw[0]:#04x}, which is not a contract")

    return raw[1:33]


def sha256(data):
    return hashlib.sha256(data).digest()


def leaf_hash(key, value):
    return sha256(b"\x00" + sha256(key) + sha256(value))


def inner_hash(left, right):
    if left == EMPTY and right == EMPTY:
        return EMPTY

    return sha256(b"\x01" + left + right)


def key_index(key):
    return int.from_bytes(sha256(key)[:8], "big")


def levels_of(leaves):
    built = [dict(leaves)]

    for _ in range(TREE_DEPTH):
        current = built[-1]
        nxt = {}

        for index in current:
            parent = index >> 1

            if parent in nxt:
                continue

            folded = inner_hash(
                current.get(parent << 1, EMPTY), current.get((parent << 1) | 1, EMPTY)
            )

            if folded != EMPTY:
                nxt[parent] = folded

        built.append(nxt)

    return built


def siblings_of(built, index):
    out = []

    for depth in range(TREE_DEPTH):
        out.append(built[depth].get((index >> depth) ^ 1, EMPTY))

    return out


def fold(index, leaf, siblings):
    current = leaf
    remaining = index

    for sibling in siblings:
        current = (
            inner_hash(current, sibling)
            if remaining & 1 == 0
            else inner_hash(sibling, current)
        )
        remaining >>= 1

    return current


def contract_data_changes(operation):
    for change in operation["changes"]:
        for body in change.values():
            if not isinstance(body, dict):
                continue

            entry = (body.get("data") or {}).get("contract_data")

            if entry:
                yield entry


def main():
    if len(sys.argv) not in (4, 5):
        fail(
            "usage: fetch-packet-witness.py <fixture.json> <transaction hash> <router C...> "
            "[path hex]"
        )

    fixture_path, transaction_hash, router_strkey = sys.argv[1:4]
    wanted_path = bytes.fromhex(sys.argv[4]) if len(sys.argv) == 5 else None

    fixture = json.load(open(fixture_path))
    network = fixture.get("network", "")

    if network not in RPC:
        fail(f"no RPC endpoint is pinned for network {network!r}")

    router = decode_contract_strkey(router_strkey)

    endpoint = RPC[network]

    print(f"rpc      {endpoint}")
    print(f"router   {router.hex()}")

    transaction = rpc(endpoint, "getTransaction", {"hash": transaction_hash})

    if transaction["status"] != "SUCCESS":
        fail(f"the transaction is {transaction['status']}, not SUCCESS")

    if transaction["ledger"] != fixture["ledger_seq"]:
        fail(
            f"the transaction is in ledger {transaction['ledger']}, "
            f"the fixture proves {fixture['ledger_seq']}"
        )

    result_index = transaction["applicationOrder"] - 1

    print(f"ledger   {transaction['ledger']}, application order {transaction['applicationOrder']}")

    pairs = fixture.get("result_pairs_xdr") or fail("the fixture carries no result pairs")

    computed = hashlib.sha256(struct.pack(">I", len(pairs)))

    for pair in pairs:
        computed.update(bytes.fromhex(pair))

    if computed.hexdigest() != fixture["tx_set_result_hash"]:
        fail("the fixture's result pairs do not hash to its txSetResultHash")

    print(f"✓ {len(pairs)} archived result pairs hash to txSetResultHash")

    pair = json.loads(
        xdr("decode", "TransactionResultPair", base64.b64encode(bytes.fromhex(pairs[result_index])).decode())
    )

    if pair["transaction_hash"] != transaction_hash:
        fail(
            f"result pair {result_index} is for {pair['transaction_hash']}, "
            f"not {transaction_hash}"
        )

    success = pair["result"]["result"]["tx_success"][0]["op_inner"]["invoke_host_function"]["success"]

    print(f"✓ result pair {result_index} is this transaction, success {success}")

    meta = json.loads(xdr("decode", "TransactionMeta", transaction["resultMetaXdr"]))
    version = next(iter(meta))
    body = meta[version]

    if version == "v4":
        events = body["operations"][0]["events"]
    elif version == "v3":
        events = body["soroban_meta"]["events"]
    else:
        fail(f"transaction meta {version} is not one this script reads")

    preimage_b64 = xdr(
        "encode",
        "InvokeHostFunctionSuccessPreImage",
        json.dumps({"return_value": body["soroban_meta"]["return_value"], "events": events}),
    )
    preimage = base64.b64decode(preimage_b64)

    if sha256(preimage).hex() != success:
        fail(
            "the preimage rebuilt from the transaction meta does not hash to the success "
            "value the ledger committed to"
        )

    print(f"✓ the rebuilt preimage hashes to {success}")

    roots = [
        bytes.fromhex(event["body"]["v0"]["data"]["bytes"])
        for event in events
        if event.get("contract_id") == router_strkey
        and [topic.get("symbol") for topic in event["body"]["v0"]["topics"]] == ["ibc_root"]
    ]

    if len(roots) != 1:
        fail(f"expected exactly one ibc_root event from the router, found {len(roots)}")

    ibc_root = roots[0]

    print(f"✓ the router published ibc_root {ibc_root.hex()}")

    leaves = None
    commitments = {}

    for entry in contract_data_changes(body["operations"][0]):
        if entry["contract"] != router_strkey:
            continue

        key = entry["key"]

        if key.get("vec") == [{"symbol": "Leaves"}]:
            leaves = {
                int(item["key"]["u64"]): bytes.fromhex(item["val"]["bytes"])
                for item in entry["val"]["map"]
            }
        elif "bytes" in key:
            commitments[bytes.fromhex(key["bytes"])] = bytes.fromhex(entry["val"]["bytes"])

    if leaves is None:
        fail("the transaction did not write the router's leaf map")

    if wanted_path is not None:
        if wanted_path not in commitments:
            written = ", ".join(key.hex() for key in commitments)
            fail(f"the transaction did not write path {wanted_path.hex()}; it wrote {written}")

        path, value = wanted_path, commitments[wanted_path]
    elif len(commitments) != 1:
        written = ", ".join(key.hex() for key in commitments)
        fail(
            f"the transaction wrote {len(commitments)} commitments ({written}); "
            "name the one to prove as a fourth argument"
        )
    else:
        path, value = next(iter(commitments.items()))
    index = key_index(path)

    if leaves.get(index) != leaf_hash(path, value):
        fail("the leaf the router stored is not the hash of the path and value it wrote")

    print(f"✓ leaf {index} is the hash of path {path.hex()} and its commitment")

    built = levels_of(leaves)
    siblings = siblings_of(built, index)

    if fold(index, leaf_hash(path, value), siblings) != ibc_root:
        fail("the proof does not fold to the published ibc_root")

    print(f"✓ the proof folds to the published ibc_root over {len(leaves)} leaf/leaves")

    fixture["packet"] = {
        "transaction_hash": transaction_hash,
        "router_contract_id": router.hex(),
        "result_index": result_index,
        "success_preimage_xdr": preimage.hex(),
        "ibc_root": ibc_root.hex(),
        "path": path.hex(),
        "value": value.hex(),
        "siblings": [one.hex() for one in siblings],
    }

    with open(fixture_path, "w") as out:
        json.dump(fixture, out, indent=2)
        out.write("\n")

    print()
    print(f"wrote the packet witness into {fixture_path}")


if __name__ == "__main__":
    main()
