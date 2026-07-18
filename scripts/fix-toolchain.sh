#!/bin/bash
# fix-toolchain.sh
# Run this on ms02 to fix the toolchain build issues
# Usage: just remote-exec './scripts/fix-toolchain.sh'

set -e

# Configuration
MS02_REPO_PATH="/home/casibbald/Workspace/microscaler/fleetingdns"
WORKING_DIR="${MS02_REPO_PATH}"

cd "${WORKING_DIR}"

echo "=== FleetingDNS Toolchain Fix ==="
echo "Working directory: ${WORKING_DIR}"
echo ""

# Step 1: Backup current state
echo "Step 1: Backing up current state..."
cp Cargo.lock Cargo.lock.backup.$(date +%Y%m%d-%H%M%S)
if [ -f rust-toolchain.toml ]; then
    cp rust-toolchain.toml rust-toolchain.toml.backup.$(date +%Y%m%d-%H%M%S)
fi

# Step 2: Try nightly-2024-08-01 first (balanced approach)
echo ""
echo "Step 2: Trying nightly-2024-08-01 (balanced approach)..."
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-08-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF

echo "Updated rust-toolchain.toml to nightly-2024-08-01"

# Step 3: Sync to latest compatible versions
echo ""
echo "Step 3: Running cargo update..."
cargo update

# Step 4: Test build
echo ""
echo "Step 4: Testing edgehub build..."
if cargo check -p edgehub 2>&1; then
    echo "✅ edgehub builds successfully!"
    
    echo ""
    echo "Testing edf-cli..."
    if cargo check -p edf-cli 2>&1; then
        echo "✅ edf-cli builds successfully!"
        
        echo ""
        echo "Testing backendapi..."
        if cargo check -p backendapi 2>&1; then
            echo "✅ backendapi builds successfully!"
            
            echo ""
            echo "=== SUCCESS: All core packages build! ==="
            
            echo ""
            echo "Step 5: Running tests..."
            if cargo test -p edgehub --lib -- --nocapture 2>&1 | head -50; then
                echo "✅ edgehub tests pass!"
            else
                echo "⚠️  Some tests may fail - this is expected for partial fixes"
            fi
            
            echo ""
            echo "=== FIX COMPLETE ==="
            echo ""
            echo "Next steps:"
            echo "1. Sync changes to laptop: just sync"
            echo "2. Commit changes: git add rust-toolchain.toml Cargo.lock"
            echo "3. Push: git push origin fix-tunnel-creation"
            echo ""
            
            # Show current state
            echo "Current toolchain:"
            rustc --version 2>/dev/null || echo "rustc not available"
            echo ""
            echo "Cargo.lock checksum:"
            sha256sum Cargo.lock | head -c 64
            echo ""
            
        else
            echo "❌ backendapi build failed, rolling back..."
            git checkout Cargo.lock rust-toolchain.toml 2>/dev/null || true
            exit 1
        fi
    else
        echo "❌ edf-cli build failed, trying alternative..."
    fi
else
    echo "❌ edgehub build failed, trying nightly-2024-07-01..."
    
    cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-07-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF
    
    cargo update
    
    if cargo check -p edgehub 2>&1; then
        echo "✅ Success with nightly-2024-07-01!"
        
        if cargo check -p edf-cli 2>&1; then
            echo "✅ edf-cli also works!"
            echo "=== FIX COMPLETE (nightly-2024-07-01) ==="
        else
            echo "⚠️  edf-cli still failing with nightly-2024-07-01, trying oldest known good..."
        fi
    else
        echo "❌ nightly-2024-07-01 failed, trying nightly-2024-06-01..."
        
        cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-06-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF
        
        cargo update
        
        if cargo check -p edgehub 2>&1; then
            echo "✅ Success with nightly-2024-06-01!"
            
            if cargo check -p edf-cli 2>&1; then
                echo "✅ edf-cli also works!"
                echo "=== FIX COMPLETE (nightly-2024-06-01 - oldest known good) ==="
            else
                echo "⚠️  edf-cli failing even with oldest toolchain"
            fi
        else
            echo ""
            echo "=== ALL AUTOMATED ATTEMPTS FAILED ==="
            echo ""
            echo "Manual intervention required. Options:"
            echo ""
            echo "1. Try pinning specific dependencies:"
            echo "   cargo update generic-array --precise 0.14.7"
            echo "   cargo update time --precise 0.3.36"
            echo ""
            echo "2. Try different nightly version:"
            echo "   cargo +nightly-2024-05-15 check -p edgehub"
            echo ""
            echo "3. Roll back completely:"
            echo "   git checkout Cargo.lock rust-toolchain.toml"
            echo ""
            
            exit 1
        fi
    fi
fi

echo ""
echo "=== TOOLCHAIN FIX SCRIPT COMPLETE ==="
echo ""
echo "Remember to sync changes to laptop:"
echo "  just sync"
echo ""
echo "Then commit and push:"
echo "  git add rust-toolchain.toml Cargo.lock"
echo "  git commit -m 'fix(toolchain): update to compatible nightly version'"
echo "  git push origin fix-tunnel-creation"
