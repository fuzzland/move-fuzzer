#!/bin/bash

set -e 

echo "[*] Aptos Demo Compilation and Fuzzing Script"

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Module Path
DEMO_CONTRACT_DIR="$PROJECT_ROOT/contracts/aptos-demo"
LIBAFL_APTOS_BIN="$PROJECT_ROOT/target/release/libafl-aptos"

echo "[*] Project root: $PROJECT_ROOT"
echo "[*] Demo contract directory: $DEMO_CONTRACT_DIR"

# Check if aptos CLI is available
if ! command -v aptos &> /dev/null; then
    echo "[-] Error: aptos CLI not found. Please install it first."
    echo "[*] You can install it with: curl -fsSL \"https://aptos.dev/scripts/install_cli.sh\" | sh"
    exit 1
fi

# Check if demo contract directory exists
if [[ ! -d "$DEMO_CONTRACT_DIR" ]]; then
    echo "[-] Error: Demo contract directory not found: $DEMO_CONTRACT_DIR"
    exit 1
fi

echo "[+] Step 1: Building libafl-aptos binary..."
cd "$PROJECT_ROOT"
cargo build --release --bin libafl-aptos

# Check if libafl-aptos binary exists
if [[ ! -f "$LIBAFL_APTOS_BIN" ]]; then
    echo "[-] Error: libafl-aptos binary not found after build: $LIBAFL_APTOS_BIN"
    exit 1
fi

echo "[+] Step 2: Compiling aptos-demo contract..."
cd "$DEMO_CONTRACT_DIR"

# Clean previous build artifacts
if [[ -d "build" ]]; then
    echo "[*] Cleaning previous build artifacts..."
    rm -rf build
fi

# Compile with all artifacts
echo "[*] Running: aptos move compile --included-artifacts all"
aptos move compile --included-artifacts all

# Check if compilation was successful
if [[ ! -d "build" ]]; then
    echo "[-] Error: Compilation failed - build directory not created"
    exit 1
fi

echo "[+] Step 3: Using default artifact paths..."

# Use default paths as specified
MODULE_PATH="$DEMO_CONTRACT_DIR/build/aptos-demo/bytecode_modules/shl_demo.mv"
ABI_PATH="$DEMO_CONTRACT_DIR/build/aptos-demo/abis"

echo "[*] Module path: $MODULE_PATH"
echo "[*] ABI path: $ABI_PATH"

# Verify the paths exist
if [[ ! -f "$MODULE_PATH" ]]; then
    echo "[-] Error: Module file not found at: $MODULE_PATH"
    exit 1
fi

if [[ ! -d "$ABI_PATH" ]]; then
    echo "[-] Error: ABI directory not found at: $ABI_PATH"
    exit 1
fi

echo "[+] Step 4: Running libafl-aptos fuzzer..."
cd "$PROJECT_ROOT"

echo "[*] Running command:"
echo "[*] timeout 20 $LIBAFL_APTOS_BIN --module-path \"$MODULE_PATH\" --abi-path \"$ABI_PATH\""
echo ""

# Run the fuzzer
timeout 20 "$LIBAFL_APTOS_BIN" --module-path "$MODULE_PATH" --abi-path "$ABI_PATH"

echo "[+] Fuzzing completed"
