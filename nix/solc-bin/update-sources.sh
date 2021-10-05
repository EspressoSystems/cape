#!/usr/bin/env nix-shell
#! nix-shell -i bash -p nix
set -euo pipefail

URL_MACOS="https://binaries.soliditylang.org/macosx-amd64/list.json"
URL_LINUX="https://binaries.soliditylang.org/linux-amd64/list.json"

SHA_LINUX=$(nix-prefetch-url $URL_LINUX);
SHA_MACOS=$(nix-prefetch-url $URL_MACOS);

cat <<EOF | tee sources.json
{
  "linux-amd64": {
    "url": "$URL_LINUX",
    "sha256": "$SHA_LINUX"
  },
  "macosx-amd64": {
    "url": "$URL_MACOS",
    "sha256": "$SHA_MACOS"
  }
}
EOF
