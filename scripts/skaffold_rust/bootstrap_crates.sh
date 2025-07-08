#!/usr/bin/env bash
# Bootstraps FleetingDNS Rust workspace
#  • libs  →  crates/<name>
#  • bins  →  crates/bin/<name>   (each depends on its sibling lib)
# All crates use edition 2024.

set -euo pipefail

#################### lists #########################################
LIBS=(
  common
  auth
  feature_pipe
  metrics_client
  dnsd            # core logic for authoritative DNS
  edgehub         # reverse-tunnel gateway logic
  intake_collector
  ml_scorer
  feed_grpc
  feed_webhook
  api
)

BINS=(            # launchers that wrap the libs
  dnsd
  edgehub
  intake_collector
  ml_scorer
  feed_grpc
  feed_webhook
  api
)
####################################################################

mkdir -p crates crates/bin

# 1) root workspace manifest if missing
if [[ ! -f Cargo.toml ]]; then
  cat > Cargo.toml <<'TOML'
[workspace]
members = []
resolver = "2"
TOML
fi

# 2) helper funcs
new_lib () {
  local name=$1
  [[ -d crates/$name ]] || cargo new --lib "crates/$name" --edition 2024
}

new_bin () {
  local name=$1
  local lib="$1"
  local bin_pkg="${name}-bin"              # unique crate name
  local dir="crates/bin/$name"
  [[ -d "$dir" ]] || cargo new --bin "$dir" --edition 2024
  # make binary depend on sibling lib
  cat >>"$dir/Cargo.toml" <<TOML

[dependencies]
$lib = { path = "../../$lib" }
TOML
  cat >"$dir/src/main.rs" <<RS
use $lib::run;

fn main() {
    run();
}
RS
}

# 3) create crates
for l in "${LIBS[@]}";  do new_lib "$l";  done
for b in "${BINS[@]}";  do new_bin "$b";  done

# 4) update workspace members
members=$(printf '  "crates/%s",\n' "${LIBS[@]}" | sed '$s/,$//')
bin_members=$(printf '  "crates/bin/%s",\n' "${BINS[@]}" | sed '$s/,$//')
all_members=$(printf '%s\n%s' "$members" "$bin_members")

# replace or add [workspace] members
if grep -q '^\[workspace\]' Cargo.toml; then
  sed -Ei '/members = \[/,/]/c\members = [\n'"$all_members"'\n]' Cargo.toml
else
  printf '\n[workspace]\nmembers = [\n%s\n]\n' "$all_members" >> Cargo.toml
fi

echo "✅  Workspace ready (libs in crates/, bins in crates/bin/, edition 2024)."
