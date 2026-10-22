#!/bin/sh
# Checks a release APK against SPEC §1 and §8.4: no network permissions, no
# system backups, and native libraries that import no socket functions; and that
# every library a native library needs is in the APK (D-23: sherpa-onnx).
#
#   tools/check-android-apk.sh <release.apk>
#
# Needs ANDROID_HOME (build-tools for aapt2) and an NDK (ANDROID_NDK_HOME or
# NDK_HOME) for llvm-nm.
set -eu
apk=$1
sdk=${ANDROID_HOME:?ANDROID_HOME is not set}
ndk=${NDK_HOME:-${ANDROID_NDK_HOME:-${ANDROID_NDK_LATEST_HOME:-}}}
[ -n "$ndk" ] || { echo "NDK_HOME or ANDROID_NDK_HOME is not set" >&2; exit 1; }

aapt2=$(ls "$sdk"/build-tools/*/aapt2 | sort -V | tail -n 1)
nm=$(ls "$ndk"/toolchains/llvm/prebuilt/*/bin/llvm-nm | head -n 1)
fail=0

permissions=$("$aapt2" dump permissions "$apk")
echo "$permissions"
for p in INTERNET ACCESS_NETWORK_STATE ACCESS_WIFI_STATE CHANGE_NETWORK_STATE; do
  if echo "$permissions" | grep -q "android.permission.$p'"; then
    echo "FAIL: the APK requests android.permission.$p" >&2
    fail=1
  fi
done

manifest=$("$aapt2" dump xmltree "$apk" --file AndroidManifest.xml)
for attr in 'allowBackup(0x01010280)=false' 'fullBackupContent(0x010104eb)=false' 'dataExtractionRules'; do
  if ! echo "$manifest" | grep -q "$attr"; then
    echo "FAIL: the manifest lacks $attr" >&2
    fail=1
  fi
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
unzip -q -o "$apk" 'lib/*' -d "$work"
readelf=$(dirname "$nm")/llvm-readelf
# Libraries every Android device provides; anything else must be in the APK.
system='libc.so libm.so libdl.so liblog.so libandroid.so'
for so in "$work"/lib/*/*.so; do
  for needed in $("$readelf" -d "$so" | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p'); do
    case " $system " in *" $needed "*) continue ;; esac
    if [ ! -f "$(dirname "$so")/$needed" ]; then
      echo "FAIL: ${so#"$work"/} needs $needed, which is not in the APK" >&2
      fail=1
    fi
  done
  imports=$("$nm" -D --undefined-only "$so" | awk '{print $NF}')
  network=$(echo "$imports" | grep -xE 'socket|connect|bind|listen|accept4?|getaddrinfo|gethostbyname|sendto|recvfrom' || true)
  if [ -n "$network" ]; then
    echo "FAIL: ${so#"$work"/} imports $(echo "$network" | tr '\n' ' ')" >&2
    fail=1
  else
    echo "ok: ${so#"$work"/} imports no socket functions"
  fi
done

[ "$fail" = 0 ] && echo "APK checks passed"
exit "$fail"
