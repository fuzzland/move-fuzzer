#!/bin/bash
# Setup Sui binary and client configuration for CI/testing environment

set -e

echo "Setting up Sui environment..."

# Check if we need to setup the Sui binary (typically in CI/Linux environment)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$OSTYPE" == "linux-gnu"* ]] && [[ -f "$SCRIPT_DIR/sui-linux.zst" ]]; then
    echo "Setting up Sui binary for Linux..."
    cd "$SCRIPT_DIR"

    # Extract the compressed binary
    if ! command -v zstd &> /dev/null; then
        echo "Error: zstd is required to extract sui-linux.zst"
        exit 1
    fi

    zstd -d -f sui-linux.zst -o sui
    chmod +x sui

    # Verify the binary
    file sui
    ls -la sui
    echo "✓ Sui binary extracted and made executable"
fi

echo "Setting up Sui client configuration..."

# Create Sui config directory
mkdir -p ~/.sui/sui_config

# Create empty keystore file
echo "[]" > ~/.sui/sui_config/sui.keystore

# Create client.yaml configuration
cat > ~/.sui/sui_config/client.yaml << 'EOF'
---
keystore:
  File: ~/.sui/sui_config/sui.keystore
external_keys: ~
envs:
  - alias: local
    rpc: "http://127.0.0.1:9000"
    ws: ~
    basic_auth: ~
active_env: local
active_address: ~
EOF

# Expand ~ to actual home path in the config
sed -i.bak "s|~/.sui/sui_config/sui.keystore|$HOME/.sui/sui_config/sui.keystore|g" ~/.sui/sui_config/client.yaml
rm ~/.sui/sui_config/client.yaml.bak

echo "✓ Sui client configuration initialized"
echo "  Config dir: ~/.sui/sui_config"
echo "  Environment: local (http://127.0.0.1:9000)"
echo "✓ Sui environment setup complete"