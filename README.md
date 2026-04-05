# solana-grpc-proxy

Minimal Pingora-based gRPC proxy for **Solana Yellowstone/Geyser** with
per-IP `SubscribeRequest` filtering.

## Features

- **Per-IP rules** loaded from TOML files in `rules_dir`
- **Unknown IP → reject** with gRPC `PERMISSION_DENIED` (no default rules)
- **Hot reload** of rules without restart (default every 30s)
- **Concurrent connection limits** per IP via `max_connections`
- **SubscribeRequest validation**: account/owner counts, reject lists, data slices, block flags, transaction filters, blocks_meta, entry
- **Proper gRPC errors**: trailers-only response with `grpc-status` and `grpc-message`
- **Zero external protobuf dependencies** — manual wire format parser
- Based directly on Pingora primitives — no pingap overhead

## Quick start

```bash
cargo build --release
./target/release/solana-grpc-proxy -c config.toml
```

## Configuration

### Main config (`config.toml`)

```toml
listen = "0.0.0.0:10000"
upstream = "127.0.0.1:11000"     # Yellowstone gRPC endpoint
rules_dir = "./rules"
reload_interval = "30s"
log_level = "info"
```

### Per-IP rules (`rules/<IP>.toml`)

See `rules/example.toml` for a full example.

- File name must match client IP exactly (e.g. `192.168.1.10.toml`)
- No file = request rejected with `PERMISSION_DENIED`

## gRPC error codes returned

| Situation | Code | Meaning |
|-----------|------|---------|
| No rules for IP | 7  | PERMISSION_DENIED |
| Too many connections | 8 | RESOURCE_EXHAUSTED |
| Invalid filter (too many accounts, rejected address, etc) | 3 | INVALID_ARGUMENT |
| Upstream/internal error | 13 | INTERNAL |

## How it works

1. Client sends gRPC `/geyser.Geyser/Subscribe` request
2. `request_filter` checks IP has a rules file, loads rules, enforces `max_connections`
3. `request_body_filter` parses the first SubscribeRequest protobuf frame and validates against per-IP rules
4. On reject: sends a gRPC trailers-only error response, drops body, closes connection — no upstream connection is forwarded
5. On pass: proxies bidirectional gRPC stream to Yellowstone upstream

## License

Apache-2.0
