# move-fuzzer

## Automated Integration Testing

```bash
# Setup venv
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/requirements.txt

# Run tests
python scripts/integration_test.py
```

The script will automatically:

1. Build move-fuzzer (cargo build --release)
2. Start a fresh localnet
3. Setup wallet and request tokens from faucet
4. Deploy the shl_demo contract
5. Create necessary test objects (shared structs)
6. Run all fuzzing test cases with 10,000 iterations each
7. Exit with code 0 if all tests pass, 1 if any fail

## Manual Testing

### Run A Localnet

- source code: `https://github.com/fuzzland/sui`
- build with tracing: `cargo build -r --features tracing`

```sh
cd scripts
RUST_LOG="off,sui_node=info" ./sui start --with-faucet --force-regenesis
```

### Setup Client

```sh
# New address and switch to it
scripts/sui client new-address ed25519 move-fuzzer
scripts/sui client switch --address move-fuzzer
scripts/sui client active-address

scripts/sui client new-env --alias local --rpc http://127.0.0.1:9000
scripts/sui client switch --env local
scripts/sui client faucet
scripts/sui client gas
```

### Deploy shl_demo

```sh
# Build shl_demo contract
cd contracts/sui-demo
../../scripts/sui move build

# Deploy to localnet
../../scripts/sui client publish --gas-budget 100000000
# package_id: 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91

cd ..

# Create shared struct (value=12, shift_amount=2)
scripts/sui client call \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function create_shared_demo_struct \
    --args 12 2
# object_id: 0xea0be34b0ec5960d42c52254cfb1bace46381c7bcae7e1f81421e2d4521bf226

# Create nested shared struct (value=15, shift_amount=3, multiplier=2)
scripts/sui client call \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function create_shared_nested_demo_struct \
    --args 15 3 2
# object_id: 0x5e1eefeb9e8fa8c6fd8bdfb56935a52525f5bf08a2c6dce2cd2f459e5ce9e0dd
```

### Fuzzing Commands

```sh
# integer args
RUST_LOG=debug cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function integer_shl \
    --args 5 2

# vector args
RUST_LOG=debug cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function vector_shl \
    --args '[5,2]'

# generic args
RUST_LOG=debug cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function generic_shl \
    --type-args u64 u8 \
    --args 5 2

# mutable shared struct
RUST_LOG=debug cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function mutable_shared_struct_shl \
    --args 0xea0be34b0ec5960d42c52254cfb1bace46381c7bcae7e1f81421e2d4521bf226

# mutable nested shared struct
RUST_LOG=debug cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91 \
    --module shl_demo \
    --function nested_mutable_shared_struct_shl \
    --args 0x5e1eefeb9e8fa8c6fd8bdfb56935a52525f5bf08a2c6dce2cd2f459e5ce9e0dd
```

### Output Example

```log
❗️ SHIFT VIOLATION DETECTED

Target Function: 0xa175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91::shl_demo::integer_shl
Iteration: 2/1000000
Execution Time: 0.08s

VIOLATION DETAILS:
- Type: Shift Overflow
- Location: a175592bdf05b7da39b2adb9d4509db89573bdca95d5a635ded388a592991a91::shl_demo::integer_shl (pc: 2)
- Value: U64(65)
- Shift Amount: 255
- Instruction: Shl

FUNCTION PARAMETERS:
- Parameter[0]: u64 = U64(65)
- Parameter[1]: u8 = U8(255)
```
