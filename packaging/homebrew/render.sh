#!/bin/sh
# Renders the cask from SHA256SUMS of a release: render.sh <version> <SHA256SUMS> <out.rb>
set -eu
version=$1 sums=$2 out=$3
sha() { awk -v f="Mossbook_${version}_$1.dmg" '$2 == f { print $1 }' "$sums"; }
arm=$(sha aarch64) intel=$(sha x64)
[ -n "$arm" ] && [ -n "$intel" ] || { echo "missing .dmg checksums for $version" >&2; exit 1; }
sed -e "s/@VERSION@/$version/" -e "s/@SHA_ARM@/$arm/" -e "s/@SHA_INTEL@/$intel/" \
  "$(dirname "$0")/mossbook.rb.template" > "$out"
