# gRPC Subscribe Filter — Тесты через grpcurl

```bash
export HOST=104.204.140.68:80
```

Замени `geyser.proto` на путь к proto файлу из yellowstone-grpc-proto.

---

## Accounts

### accounts: account filter
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA","9vruaxeU4HEnj8czkpdyYeScnerqoEAjpuGBHh7kNKbV"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts: owner filter
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"owner":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts: account_reject (should be rejected)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts: owner_reject (should be rejected)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"owner":["11111111111111111111111111111111"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts: exceed account_max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA","9vruaxeU4HEnj8czkpdyYeScnerqoEAjpuGBHh7kNKbV","EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB","So11111111111111111111111111111111111111112"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts: exceed owner_max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"owner":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL","11111111111111111111111111111111"]}}}' $HOST geyser.Geyser/Subscribe
```

### accounts_data_slice: exceed data_slice_max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}},"accounts_data_slice":[{"offset":0,"length":32},{"offset":32,"length":32},{"offset":64,"length":32},{"offset":96,"length":32}]}' $HOST geyser.Geyser/Subscribe
```

---

## Transactions

### transactions: account_include
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions: account_exclude
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"account_exclude":["Vote111111111111111111111111111111111111111","gqXIqGo1ZpSscSlkLCScmW2ero4ovTTwM1CKbjKQHp4"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions: account_required
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"account_required":["EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions: account_include_reject (should be rejected)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions":{"filter":{"account_include":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions: exceed account_include_max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA","9vruaxeU4HEnj8czkpdyYeScnerqoEAjpuGBHh7kNKbV","EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB","So11111111111111111111111111111111111111112"]}}}' $HOST geyser.Geyser/Subscribe
```

---

## Transactions Status

### transactions_status: account_include
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions_status":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions_status: account_exclude
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions_status":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"account_exclude":["Vote111111111111111111111111111111111111111"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions_status: account_required
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions_status":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"account_required":["EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"]}}}' $HOST geyser.Geyser/Subscribe
```

### transactions_status: account_include_reject (should be rejected)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"transactions_status":{"filter":{"account_include":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]}}}' $HOST geyser.Geyser/Subscribe
```

---

## Blocks

### blocks: account_include
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}}}' $HOST geyser.Geyser/Subscribe
```

### blocks: include_transactions = true (should pass if allowed)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"include_transactions":true}}}' $HOST geyser.Geyser/Subscribe
```

### blocks: include_accounts = true (should be rejected if not allowed)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"include_accounts":true}}}' $HOST geyser.Geyser/Subscribe
```

### blocks: include_entries = true (should be rejected if not allowed)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks":{"filter":{"account_include":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"],"include_entries":true}}}' $HOST geyser.Geyser/Subscribe
```

### blocks: account_include_reject (should be rejected)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks":{"filter":{"account_include":["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]}}}' $HOST geyser.Geyser/Subscribe
```

---

## Blocks Meta

### blocks_meta: within limit
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks_meta":{"filter1":{},"filter2":{}}}' $HOST geyser.Geyser/Subscribe
```

### blocks_meta: exceed max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"blocks_meta":{"f1":{},"f2":{},"f3":{},"f4":{},"f5":{},"f6":{},"f7":{},"f8":{},"f9":{},"f10":{},"f11":{}}}' $HOST geyser.Geyser/Subscribe
```

---

## Entry

### entry: within limit
```bash
grpcurl -plaintext -proto geyser.proto -d '{"entry":{"filter1":{},"filter2":{}}}' $HOST geyser.Geyser/Subscribe
```

### entry: exceed max (should be rejected if max < count)
```bash
grpcurl -plaintext -proto geyser.proto -d '{"entry":{"f1":{},"f2":{},"f3":{},"f4":{},"f5":{},"f6":{},"f7":{},"f8":{},"f9":{},"f10":{},"f11":{}}}' $HOST geyser.Geyser/Subscribe
```

---

## Connection Limits

### max_connections: open multiple streams from same IP
```bash
for i in $(seq 1 6); do grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}}}' $HOST geyser.Geyser/Subscribe & done; wait
```

---

## IP without config file (should be rejected immediately)

```bash
grpcurl -plaintext -proto geyser.proto -d '{"accounts":{"filter":{"account":["pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"]}}}' $HOST geyser.Geyser/Subscribe
```
Expected: `Code: PermissionDenied` / connection reset.

---

## Sample per-IP config for testing

```toml
# rules/104.204.140.68.toml
max_connections = 5

[accounts]
account_max = 3
owner_max = 2
data_slice_max = 2
account_reject = ["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]
owner_reject = ["11111111111111111111111111111111"]

[transactions]
account_include_max = 3
account_exclude_max = 3
account_required_max = 3
account_include_reject = ["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]

[blocks]
account_include_max = 3
include_accounts = false
include_entries = false
include_transactions = true
account_include_reject = ["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]

[blocks_meta]
max = 10

[entry]
max = 10

[transactions_status]
account_include_max = 3
account_exclude_max = 3
account_required_max = 3
account_include_reject = ["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]
```
