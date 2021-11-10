#!/usr/bin/env nix-shell
#! nix-shell -i bash -p nix
set -euo pipefail

URL_MACOS="https://binaries.soliditylang.org/macosx-amd64/list.json"
URL_LINUX="https://binaries.soliditylang.org/linux-amd64/list.json"

wget $URL_MACOS -O list-macosx-amd64.json
wget $URL_LINUX -O list-linux-amd64.json
