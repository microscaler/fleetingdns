# Toolchain Fix for FleetingDNS

## Problem

The current build fails on `nightly` toolchain due to:
1. `generic-array 0.14.7` incompatibility with recent nightlies
2. `http-body` crate failing to resolve `http` crate
3. `getrandom` version conflicts between `rand_core 0.6.x` and `rand_core 0.9.x`
4. `icu_*` crates requiring Rust 1.86+
5. `time` crate requiring Rust 1.88+

## Root Cause Analysis

The `Cargo.lock` was generated with a specific nightly version that is now incompatible with newer dependency versions. The workspace has:
- `edition = "2024"` in some crates (requires newer Rust)
- Various dependencies that have released newer versions requiring newer Rust

## Solution Options

### Option 1: Pin to Compatible Nightly (Recommended)

Pin to `nightly-2024-06-01` which is known to work with the existing `Cargo.lock`:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2024-06-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
```

**Pros:**
- Works with existing `Cargo.lock`
- Stable, known-good version
- Minimal changes required

**Cons:**
- Older toolchain (June 2024)
- May miss some newer Rust features
- Dependencies may be outdated

### Option 2: Update Dependencies (Cleaner Long-term)

Update `Cargo.lock` to compatible versions that work with current nightly:

```bash
# On ms02
cd /home/casibbald/Workspace/microscaler/fleetingdns
cargo update
cargo check -p edgehub
# If it works, commit the new Cargo.lock
```

**Pros:**
- Uses latest Rust
- Dependencies are current
- Better for future maintenance

**Cons:**
- Risk of breaking other dependencies
- May require additional code changes
- Longer testing cycle

### Option 3: Hybrid Approach (Best of Both)

Use a slightly newer nightly than option 1 but older than the problematic versions:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2024-08-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
```

**Pros:**
- Balance between stability and currency
- Still likely compatible with current deps
- More recent than option 1

**Cons:**
- May still have compatibility issues
- Requires experimentation

## Step-by-Step Fix Script

Run this on **ms02** (the dev host where Tilt runs):

```bash
#!/bin/bash
# fix-toolchain.sh - Run on ms02

set -e

cd /home/casibbald/Workspace/microscaler/fleetingdns

echo "=== FleetingDNS Toolchain Fix ==="
echo ""

# Option 1: Try to use nightly-2024-08-01 (balanced approach)
echo "Step 1: Updating rust-toolchain.toml..."
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-08-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF

echo "rust-toolchain.toml updated to nightly-2024-08-01"

# Backup old Cargo.lock
echo ""
echo "Step 2: Backing up Cargo.lock..."
cp Cargo.lock Cargo.lock.backup.$(date +%Y%m%d-%H%M%S)

# Step 3: Sync to latest compatible versions
echo ""
echo "Step 3: Running cargo update to find compatible versions..."
cargo update

# Step 4: Test build
echo ""
echo "Step 4: Testing edgehub build..."
cargo check -p edgehub

if [ $? -eq 0 ]; then
    echo ""
    echo "=== SUCCESS: edgehub builds successfully! ==="
    echo ""
    echo "Step 5: Testing edf-cli..."
    cargo check -p edf-cli
    
    if [ $? -eq 0 ]; then
        echo ""
        echo "=== SUCCESS: All packages build! ==="
        echo ""
        echo "Step 6: Committing changes..."
        echo "git add rust-toolchain.toml Cargo.lock"
        echo "git commit -m 'fix(toolchain): update to compatible nightly version'"
        echo ""
        echo "Remember to sync to laptop:"
        echo "git push origin fix-tunnel-creation"
    else
        echo ""
        echo "=== edf-cli build failed, rolling back ==="
        echo "git checkout Cargo.lock"
    fi
else
    echo ""
    echo "=== edgehub build failed, rolling back ==="
    echo "git checkout Cargo.lock"
    echo ""
    echo "Trying alternative: nightly-2024-07-01..."
    
    cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-07-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF
    
    cargo update
    cargo check -p edgehub
    
    if [ $? -eq 0 ]; then
        echo "=== Success with nightly-2024-07-01 ==="
    else
        echo "=== All attempts failed, trying nightly-2024-06-01 ==="
        
        cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-06-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF
        
        cargo update
        cargo check -p edgehub
        
        if [ $? -eq 0 ]; then
            echo "=== Success with nightly-2024-06-01 (oldest known good) ==="
        else
            echo "=== ALL ATTEMPTS FAILED ==="
            echo ""
            echo "Please check the following:"
            echo "1. Check rustc version: rustc --version"
            echo "2. Check error details above"
            echo "3. Consider updating specific dependencies manually"
            echo ""
            echo "Manual fixes that may help:"
            echo "- cargo update generic-array@<current> --precise 0.14.7"
            echo "- cargo update time@<current> --precise 0.3.36"
            echo "- cargo update icu_*@<current> --precise 1.5.0"
            exit 1
        fi
    fi
fi

```

## Sync Instructions

After fixing on ms02:

### From Laptop:
```bash
# Sync the changes to your local copy
just sync

# Pull the latest from the branch
git pull origin fix-tunnel-creation

# Verify you have the fix
cat rust-toolchain.toml
cat Cargo.lock | grep -A1 "^name = \"generic-array\""
```

### Verify the Fix:
```bash
# Check rust version
rustc --version

# Try building
cargo check -p edgehub
cargo check -p edf-cli

# Run tests
cargo test -p edgehub --test e2e_reverse_tunnel_http
```

## Rollback Instructions

If the fix doesn't work:

```bash
# Restore old Cargo.lock
git checkout Cargo.lock

# Or try a different nightly
cat > rust-toolchain.toml << 'EOF'
[toolchain]
channel = "nightly-2024-05-01"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
EOF
```

## Manual Dependency Updates

If you need to manually update specific packages:

```bash
# Update generic-array to fix crypto_common issues
cargo update generic-array@0.14.7 --precise 0.14.7

# Update time crate to fix Rust version requirement
cargo update time@0.3.36 --precise 0.3.36

# Update icu crates if needed
cargo update icu_collections@2.2.0 --precise 1.5.0
cargo update icu_locale_core@2.2.0 --precise 1.5.0
cargo update icu_normalizer@2.2.0 --precise 1.5.0
cargo update icu_properties@2.2.0 --precise 1.5.0
cargo update icu_provider@2.2.0 --precise 1.5.0

# Update time-core and time-macros
cargo update time-core@0.1.2 --precise 0.1.2
cargo update time-macros@0.2.18 --precise 0.2.18
```

## Verification Checklist

After applying the fix, verify:

- [ ] `cargo check -p edgehub` succeeds
- [ ] `cargo check -p edf-cli` succeeds
- [ ] `cargo check -p backendapi` succeeds
- [ ] `cargo test -p edgehub` passes
- [ ] `cargo test -p edf-cli` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --all` runs without changes needed

## Notes

1. **Never build on the laptop** - All builds must happen on ms02 where the shared Kind cluster and Docker daemon live.

2. **Always sync changes** - Use `just sync` to sync from ms02 to laptop after making changes.

3. **Check Rust version** - Verify `rustc --version` matches expectations before building.

4. **Commit Cargo.lock** - Always commit the `Cargo.lock` file when updating dependencies.

5. **Test before committing** - Run tests locally (synced from ms02) before pushing.
