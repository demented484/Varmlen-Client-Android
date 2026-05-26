#!/usr/bin/env bash
# Remove the AegisVPN privileged helper. Run as root.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "error: must run as root (sudo $0)" >&2
  exit 1
fi

systemctl disable --now aegisvpn-helper.service 2>/dev/null || true
rm -f /etc/systemd/system/aegisvpn-helper.service
rm -rf /usr/local/lib/aegisvpn
systemctl daemon-reload

echo "AegisVPN helper removed."
