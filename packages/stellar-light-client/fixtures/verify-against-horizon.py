#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0

import base64
import json
import pathlib
import struct
import sys
import urllib.request

HOSTS = {"mainnet": "horizon.stellar.org", "testnet": "horizon-testnet.stellar.org"}


class Reader:
    def __init__(self, raw):
        self.raw = raw
        self.at = 0

    def u32(self):
        value = struct.unpack_from(">I", self.raw, self.at)[0]
        self.at += 4
        return value

    def u64(self):
        value = struct.unpack_from(">Q", self.raw, self.at)[0]
        self.at += 8
        return value

    def fixed(self, size):
        value = self.raw[self.at : self.at + size]
        self.at += size
        return value

    def opaque(self):
        size = self.u32()
        padding = (4 - size % 4) % 4
        value = self.raw[self.at : self.at + size]
        self.at += size + padding
        return value


def parse_ledger_header(raw):
    reader = Reader(raw)

    reader.u32()
    previous_ledger_hash = reader.fixed(32)

    reader.fixed(32)
    close_time = reader.u64()

    for _ in range(reader.u32()):
        reader.opaque()

    extension = reader.u32()

    if extension == 1:
        reader.u32()
        reader.fixed(32)
        reader.opaque()
    elif extension != 0:
        raise SystemExit(f"unexpected StellarValue ext discriminant {extension}")

    tx_set_result_hash = reader.fixed(32)
    bucket_list_hash = reader.fixed(32)
    ledger_seq = reader.u32()

    return {
        "previous_ledger_hash": previous_ledger_hash.hex(),
        "close_time": close_time,
        "tx_set_result_hash": tx_set_result_hash.hex(),
        "bucket_list_hash": bucket_list_hash.hex(),
        "ledger_seq": ledger_seq,
    }


def check(path):
    fixture = json.load(open(path))

    network = fixture.get("network") or "mainnet"
    sequence = fixture["ledger_seq"]
    host = HOSTS[network]

    with urllib.request.urlopen(f"https://{host}/ledgers/{sequence}", timeout=30) as response:
        horizon = json.load(response)

    raw = base64.b64decode(horizon["header_xdr"])
    parsed = parse_ledger_header(raw)

    results = [
        ("the header bytes are the ones Stellar published", raw == bytes.fromhex(fixture["ledger_header_xdr"])),
        ("the ledgerSeq anchor lands where it should", parsed["ledger_seq"] == horizon["sequence"]),
        ("the ledger hash matches horizon", fixture["ledger_hash"] == horizon["hash"]),
        ("the previous ledger hash matches horizon", parsed["previous_ledger_hash"] == horizon["prev_hash"]),
        ("the close time matches the fixture", parsed["close_time"] == fixture["close_time"]),
        ("the result hash matches the fixture", parsed["tx_set_result_hash"] == fixture["tx_set_result_hash"]),
        ("the bucket list hash matches the fixture", parsed["bucket_list_hash"] == fixture.get("bucket_list_hash")),
    ]

    print(f"--- {network} {sequence} ({path.name}) ---")

    for what, passed in results:
        print(f"  {'ok  ' if passed else 'FAIL'} {what}")

    if not all(passed for _, passed in results):
        print(f"  parsed bucket list hash: {parsed['bucket_list_hash']}")

    return all(passed for _, passed in results)


def main():
    here = pathlib.Path(__file__).parent

    paths = [pathlib.Path(one) for one in sys.argv[1:]] or sorted(here.glob("*.json"))

    if all(check(path) for path in paths):
        print("\nevery fixture agrees with what Stellar published")
        return 0

    print("\na fixture disagrees with what Stellar published")
    return 1


if __name__ == "__main__":
    sys.exit(main())
