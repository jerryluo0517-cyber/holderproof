# Step 1: Lyquor Environment Verification

Date: 2026-11-11  
Status: **PASS with one scope adjustment**

## Installed toolchain

| Tool | Version / result |
|---|---|
| Rust | 1.99.0 |
| WASM target | `wasm32-unknown-unknown` installed |
| Foundry | 1.7.1 |
| Lyquor | LDK release `lyquor-2026-11-10-54c5e11` |
| shaker | 0.8.4+54c5e11 |
| LDK platform version | 0.8.4 |

Install location: `~/.shakenup`.

## Devnet and Hello Lyquid

The single-node native localnet started successfully:

- API: `http://127.0.0.1:10087/api`
- WebSocket: `ws://127.0.0.1:10087/ws`

The official `hello` example was built and deployed:

```text
Lyquid ID: Lyquid-ahjg4huwvban3bvhxue7rcbaw4mnltwyolhqa
Contract:  0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
Platform:  0.8
```

A `greet()` transaction was accepted by the local Anvil-backed backend, proving that the
deployment and Ethereum-export input path are functional.

## Confirmed LDK capabilities

The installed `price-feed` example is the closest official reference architecture for
HolderProof. It confirms the following APIs:

1. `network oracle <topic>;` declares a committee-backed oracle topic.
2. `configure_committee(node_ids)` computes `node_ids.len() / 2 + 1` as the threshold.
3. `propose_and_certify(...)` collects committee observations and produces a certified call.
4. `submit_certified_call!(call)` submits the certified result to the sequence backend.
5. A certified network function updates shared network state.
6. Network functions can call another Lyquid's network functions with `call!`, providing
   atomic composability.
7. UPC (`upc!` plus prepare/request/response handlers) supports direct inter-node calls when
   node-local responses are required.

## Scope adjustment

The official native multi-node devnet starts **four** nodes. Therefore HolderProof should use:

```text
Nodes:     4
Threshold: 3-of-4 (n/2 + 1)
```

This is stronger than the original 2-of-3 assumption and avoids maintaining a custom
three-node topology.

## Implementation direction

Use `~/.shakenup/ldk/lyquid-examples/price-feed` as the reference, but replace external
price fetching with deterministic ERC-20 Transfer replay:

```text
Price Feed:
fetch external prices -> committee proposal -> median -> certified update

HolderProof:
replay Transfer events -> committee proposal -> agreement check -> certified balance result
```

The first HolderProof network function should accept a query certificate/call that updates:

```rust
network holder_balance: HashMap<(Address, Address, u64), u64>;

fn update_balance(ctx: &mut _, token: Address, holder: Address, block: u64, balance: U256) { ... }
```

The consumer Lyquid should read that certified result through a network function call, not
through a direct EVM `eth_call` to an off-chain instance method.

## Gotcha

Generated Solidity stubs for instance/off-chain methods can return empty values when called
directly with `eth_call`; they are not the authoritative result path. Use the Lyquid's HTTP
exports, application-level function calls, or certified network state as specified by the LDK.

## Next gate

Before implementing the indexer, prove this minimal composition:

```text
Lyquid A certified update
        |
        v
shared network state
        |
        v
Lyquid B call! reads the result and returns eligible / not eligible
```

This is the P0 architectural gate.
