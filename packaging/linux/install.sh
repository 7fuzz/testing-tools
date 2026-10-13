#!/usr/bin/env bash
# Installs the release build as `testing-tools` for the current user, with a launcher entry and icons.
# Usage: packaging/linux/install.sh   (run from anywhere; builds first if needed)
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"

cargo build --release -p tt-desktop --manifest-path "$repo/Cargo.toml"

install -Dm755 "$repo/target/release/testing-tools" "$bin_dir/testing-tools"
for size in 32 128 256 512; do
    install -Dm644 "$repo/crates/tt-desktop/assets/icons/icon-$size.png" "$data_dir/icons/hicolor/${size}x${size}/apps/testing-tools.png"
done
# Absolute Exec path, so the launcher works even when ~/.local/bin isn't on the session PATH.
sed "s|^Exec=.*|Exec=$bin_dir/testing-tools|" "$repo/packaging/linux/testing-tools.desktop" \
    | install -Dm644 /dev/stdin "$data_dir/applications/testing-tools.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$data_dir/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$data_dir/icons/hicolor" || true

echo "Installed $bin_dir/testing-tools"
