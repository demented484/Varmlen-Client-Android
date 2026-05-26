#!/usr/bin/env bash
# Install the AegisVPN privileged helper + sing-box core as a root systemd
# service. Run once with root (the GUI invokes this via pkexec):
#
#   sudo ./install.sh /path/to/sing-box
#
# The sing-box path is the core the client already downloaded (Settings shows
# its version); it is copied to a root-owned location so the helper never runs
# a user-writable binary.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX=/usr/local/lib/aegisvpn
UNIT=/etc/systemd/system/aegisvpn-helper.service

if [[ $EUID -ne 0 ]]; then
  echo "error: must run as root (sudo $0 <sing-box-path>)" >&2
  exit 1
fi

CORE_SRC="${1:-}"
if [[ -z "$CORE_SRC" || ! -f "$CORE_SRC" ]]; then
  echo "usage: $0 <path-to-sing-box-binary>" >&2
  exit 1
fi

# The user who invoked us (via pkexec or sudo) — only they may control the VPN.
INVOKING_UID="${PKEXEC_UID:-${SUDO_UID:-}}"
if [[ -z "$INVOKING_UID" ]]; then
  echo "error: cannot determine the invoking user's UID" >&2
  exit 1
fi

HELPER_BIN="$SCRIPT_DIR/target/release/aegisvpn-helper"
if [[ ! -x "$HELPER_BIN" ]]; then
  # Don't build here — this runs as root (pkexec); building would create
  # root-owned artifacts. Build it once as your normal user beforehand:
  echo "error: helper not built. Run (as your user): (cd '$SCRIPT_DIR' && cargo build --release)" >&2
  exit 1
fi

install -Dm755 -o root -g root "$HELPER_BIN" "$PREFIX/aegisvpn-helper"
install -Dm755 -o root -g root "$CORE_SRC"  "$PREFIX/sing-box"

sed "s/__UID__/$INVOKING_UID/" "$SCRIPT_DIR/aegisvpn-helper.service" > "$UNIT"

systemctl daemon-reload
systemctl enable --now aegisvpn-helper.service

echo "AegisVPN helper installed and running (allowed UID: $INVOKING_UID)."
