#!/bin/sh
# Fetches the prebuilt sherpa-onnx + onnxruntime libraries for voice note
# transcription (D-23) and checks them against the SHA-256 sums pinned in
# tools/sherpa-onnx.sha256, then unpacks them where the build expects them. The
# build reads the archives from target/sherpa-onnx-archives (SHERPA_ONNX_ARCHIVE_DIR
# in .cargo/config.toml), so the sherpa-onnx-sys build script never downloads
# anything itself. Only the build machine connects here;
# the app never does.
#
#   tools/fetch-sherpa-onnx.sh [android|linux-x64|osx-arm64|osx-x64|win-x64|all]...
#
# Without an argument, fetches the archive for this machine. For Android it also
# unpacks the two shared libraries the app loads (the C API and ONNX Runtime) into
# target/sherpa-onnx-android/jniLibs, which Gradle packages into the APK.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
sums="$root/tools/sherpa-onnx.sha256"
dir="$root/target/sherpa-onnx-archives"
version=1.13.8
base="https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version"

if [ $# -eq 0 ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) set -- osx-arm64 ;;
    Darwin-x86_64) set -- osx-x64 ;;
    Linux-x86_64) set -- linux-x64 ;;
    MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) set -- win-x64 ;;
    *) echo "unsupported machine: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
  esac
fi

names=
for which in "$@"; do
  case "$which" in
    all) found=$(awk '{print $2}' "$sums") ;;
    *) found=$(awk '{print $2}' "$sums" | grep -e "-$which[.-]" || true) ;;
  esac
  [ -n "$found" ] || { echo "no pinned archive for $which" >&2; exit 1; }
  names="$names $found"
done

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

mkdir -p "$dir"
for name in $names; do
  want=$(grep "  $name\$" "$sums" | cut -d' ' -f1)
  file="$dir/$name"
  if [ -f "$file" ] && [ "$(sha256 "$file")" = "$want" ]; then
    echo "ok (cached): $name"
    continue
  fi
  curl --fail --location --silent --show-error --output "$file.part" "$base/$name"
  got=$(sha256 "$file.part")
  if [ "$got" != "$want" ]; then
    rm -f "$file.part"
    echo "FAIL: $name has SHA-256 $got, expected $want" >&2
    exit 1
  fi
  mv "$file.part" "$file"
  echo "ok: $name"
done

# Unpack each checked archive where the sherpa-onnx-sys 1.13.8 build script looks
# for it (target/sherpa-onnx-prebuilt/<archive>/lib, or jniLibs/<abi> for Android).
# CI restores a cached build without that folder, and the build script does not
# run again to recreate it, so the link would fail without this.
prebuilt="$root/target/sherpa-onnx-prebuilt"
mkdir -p "$prebuilt"
for name in $names; do
  case "$name" in
    *-android.tar.bz2) rm -rf "$prebuilt/jniLibs" ;;
    *) rm -rf "$prebuilt/${name%.tar.bz2}" ;;
  esac
  tar -xjf "$dir/$name" -C "$prebuilt"
done
echo "ok: unpacked into target/sherpa-onnx-prebuilt"

case "$names" in
  *-android.tar.bz2*)
    jni="$root/target/sherpa-onnx-android"
    rm -rf "$jni"
    mkdir -p "$jni"
    tar -xjf "$dir/sherpa-onnx-v$version-android.tar.bz2" -C "$jni" \
      --include '*/libsherpa-onnx-c-api.so' --include '*/libonnxruntime.so' 2>/dev/null ||
      tar -xjf "$dir/sherpa-onnx-v$version-android.tar.bz2" -C "$jni" \
        --wildcards '*/libsherpa-onnx-c-api.so' '*/libonnxruntime.so'
    echo "ok: Android libraries in target/sherpa-onnx-android/jniLibs"
    ;;
esac
