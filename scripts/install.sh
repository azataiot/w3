#!/bin/sh
set -eu

repo="azataiot/w3"
version="${W3_VERSION:-}"

if [ -n "$version" ]; then
  release="download/v$version"
else
  release="latest/download"
fi

installer=$(mktemp)
trap 'rm -f "$installer"' EXIT
if ! curl --proto '=https' --tlsv1.2 -fsSL -o "$installer" \
  "https://github.com/$repo/releases/$release/w3-cli-installer.sh"; then
  echo "install.sh: no ${version:-stable} release of $repo. Set W3_VERSION to install a pre-release." >&2
  exit 1
fi
sh "$installer" "$@"
