# Fast transfer

`FastTransfer` is the Ethereum half of ipsa's fast path. A transfer from Sepolia to Stellar waits about 13 minutes for Ethereum finality before the Stellar light client can accept it. With the fast path, a liquidity provider pays the recipient on Stellar within about a minute, from its own vouchers, and the proven IBC packet repays the provider about 20 minutes later. No token is minted or released except by a proven packet; the provider carries the timing and reorg risk.

The Stellar half is the fill contract, `ibc-fast-fill` in [`ipsa-contracts`](https://github.com/ipsadev/ipsa-contracts).

## Sending

`fastTransfer(token, amount, recipient, fee, fillDeadline)` pulls `amount` of `token` from the caller and sends it with `ICS20Transfer.sendTransfer`, with this contract as the packet's sender and the Stellar fill contract as its receiver. The memo names the fill contract as the destination callback and carries the terms:

```json
{"dest_callback":{"address":"C…FILL"},"fast_fill":"G…RECIPIENT:FEE:DEADLINE"}
```

- `recipient` must be a 56-character Stellar address (`A–Z`, `2–7`), so it cannot break the memo.
- `fee` must be below `amount`. The recipient receives `amount − fee` from the provider; the provider later receives `amount` from the packet.
- `fillDeadline` must be in the future and before the packet's timeout (`PACKET_TIMEOUT_SECONDS` after the send).

The contract emits `OrderOpened` with the order key, the packet sequence, denomination, amount, recipient, fee, deadline and the exact memo, which is everything a provider needs to fill on Stellar.

## The order key

```text
orderKey = sha256(abi.encodePacked(uint64 sequence, uint256 amount, sha256(source client), sha256(denom), sha256(memo)))
```

The Stellar fill contract computes the same key, and both test suites assert the same fixed vector (`test_orderKeyMatchesTheStellarVector` here, `the_order_key_matches_the_vector_the_sepolia_tests_assert` in `ibc-fast-fill`). The denomination is the one `ICS20Transfer` puts in the packet: the token's lowercase hex address for a native ERC-20, or its full trace for an IBC voucher.

## Outcomes and refunds

The contract implements `IIBCSenderCallbacks`, so `ICS20Transfer` reports each packet's outcome to it. Only `ICS20Transfer` may call the callbacks, and they never revert for an unknown packet, so a relay is never blocked.

- **Delivered:** a success acknowledgement marks the order delivered. The fill contract on Stellar has already paid the provider or the recipient.
- **Failed or timed out:** `ICS20Transfer` refunds the tokens to this contract, which marks the order refunded and records the time.
- **Fill notice:** if a provider filled the order, the Stellar fill contract sends `assignRefund(orderKey, provider)` through ICS-27. Only the fill contract's ICS-27 account may call it, `NOTICE_ACCOUNT`, which is derived at construction from the client, the fill contract's address and an empty salt. A notice can arrive before or after the refund, and a notice for a packet that succeeds is harmless.
- **Claiming:** anyone may call `claimRefund(orderKey)`. It pays the provider named by the notice, or the user once `GRACE_PERIOD_SECONDS` have passed since the refund with no notice. A notice that arrives after the user has claimed is too late, so the grace period must cover the worst case of the Stellar → Sepolia GMP route with margin.

## Configuration

All configuration is fixed at construction: the ICS-20 app, the ICS-27 app (used once, to derive `NOTICE_ACCOUNT`), the local client for Stellar, the fill contract's address, the packet timeout and the grace period. A change means deploying a new contract.

Fee-on-transfer and rebasing tokens are not supported: the contract assumes it receives exactly `amount`.

## Tests

`test/fast-transfer/FastTransferTest.t.sol` runs against the real `ICS26Router` and `ICS20Transfer` with the dummy light client: the packet and its memo, delivery, error acknowledgements and timeouts, notices before and after the refund, the grace period, and every refusal.
