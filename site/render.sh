#!/bin/sh
# Renders the website into a directory ready for GitHub Pages:
#   site/render.sh <version> <out-dir>
# The only template value is {{version}} (download links and labels).
set -eu
version=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
case $version in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "render.sh: version must look like 1.2.3, got '$version'" >&2; exit 1 ;;
esac
rm -rf "$out"
mkdir -p "$out"
(cd "$here" && tar cf - --exclude render.sh --exclude README.md .) | (cd "$out" && tar xf -)
find "$out" -name '*.html' | while read -r page; do
  sed "s/{{version}}/$version/g" "$page" > "$page.tmp" && mv "$page.tmp" "$page"
done
if grep -rl '{{' "$out" --include='*.html' >/dev/null 2>&1; then
  echo "render.sh: unreplaced template values left" >&2
  exit 1
fi
