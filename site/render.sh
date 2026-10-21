#!/bin/sh
# Renders the website into a directory ready for GitHub Pages:
#   site/render.sh <version> <out-dir> [android]
# The only template value is {{version}} (download links and labels).
# Text between <!--android--> and <!--/android--> is kept only with `android`
# (the release has an APK); text between <!--pre-android--> and
# <!--/pre-android--> only without it.
set -eu
version=$1
out=$2
android=${3:-}
here=$(cd "$(dirname "$0")" && pwd)
case $version in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "render.sh: version must look like 1.2.3, got '$version'" >&2; exit 1 ;;
esac
rm -rf "$out"
mkdir -p "$out"
(cd "$here" && tar cf - --exclude render.sh --exclude README.md .) | (cd "$out" && tar xf -)
if [ "$android" = android ]; then
  keep=android drop=pre-android
else
  keep=pre-android drop=android
fi
find "$out" -name '*.html' | while read -r page; do
  sed "s/{{version}}/$version/g" "$page" |
    perl -0pe "s/<!--$drop-->.*?<!--\/$drop-->//gs; s/<!--\/?$keep-->//g" > "$page.tmp"
  mv "$page.tmp" "$page"
done
if grep -rlE '\{\{|<!--/?(pre-)?android-->' "$out" --include='*.html' >/dev/null 2>&1; then
  echo "render.sh: unreplaced template values left" >&2
  exit 1
fi
