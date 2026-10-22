#!/usr/bin/env python3
"""Collect the licence texts of everything shipped inside the Mossbook app.

GPLv3 section 5(d) asks the app to show its licence, and MIT, BSD, ISC,
Apache-2.0 and OFL require their notices to travel with the binary, so the app
shows this file under Settings -> About. It starts with Mossbook's own licence
(LICENSE-EXCEPTION.md, LICENSE), followed by:

- Rust: normal (non-dev, non-build) dependencies of the `mossbook` crate,
  read from `cargo metadata` and the licence files in each crate's directory;
- npm: non-dev packages from app/package-lock.json and their licence files;
- fonts embedded in PDF reports (core/data/fonts);
- native code inside the prebuilt sherpa-onnx libraries for voice note
  transcription, which ship no licence files (core/data/licenses, D-23).

Identical licence texts are printed once, with the list of packages that use
them. Needs no network once `cargo fetch` and `npm ci` have run.

    python3 tools/licenses.py          # rewrite the file
    python3 tools/licenses.py --check  # fail if it is out of date
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "app/src/lib/assets/licenses.txt"
APP_CRATE = "mossbook"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|OFL)([-_.].*)?$", re.IGNORECASE)


def licence_texts(directory: pathlib.Path) -> list[str]:
    texts = []
    for f in sorted(directory.iterdir()):
        if f.is_file() and LICENSE_FILE.match(f.name):
            texts.append(f.read_text(encoding="utf-8", errors="replace").strip())
    return texts


def rust_packages() -> list[tuple[str, str, list[str]]]:
    meta = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT
        )
    )
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    workspace = set(meta["workspace_members"])
    start = next(i for i in workspace if by_id[i]["name"] == APP_CRATE)

    seen, stack = set(), [start]
    while stack:
        node = nodes[stack.pop()]
        for dep in node["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]) and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])

    out = []
    for pid in seen - workspace:
        p = by_id[pid]
        texts = licence_texts(pathlib.Path(p["manifest_path"]).parent)
        out.append((f"{p['name']} {p['version']}", p.get("license") or "see licence text", texts))
    return out


def npm_packages() -> list[tuple[str, str, list[str]]]:
    app = ROOT / "app"
    lock = json.loads((app / "package-lock.json").read_text())
    out = []
    for path, info in lock["packages"].items():
        if not path or info.get("dev") or info.get("devOptional"):
            continue
        directory = app / path
        manifest = json.loads((directory / "package.json").read_text())
        licence = info.get("license") or manifest.get("license") or "see licence text"
        if isinstance(licence, dict):
            licence = licence.get("type", "see licence text")
        out.append((f"{manifest['name']} {manifest['version']}", licence, licence_texts(directory)))
    return out


def font_packages() -> list[tuple[str, str, list[str]]]:
    fonts = ROOT / "core/data/fonts"
    return [("Noto Sans (PDF reports)", "OFL-1.1", licence_texts(fonts))]


# Native code inside the prebuilt sherpa-onnx archives (tools/sherpa-onnx.sha256),
# which carry no licence files; sherpa-onnx itself is covered by its crate.
NATIVE = [
    ("ONNX Runtime (in sherpa-onnx)", "MIT", "sherpa-onnx/onnxruntime"),
    ("kaldi-native-fbank (in sherpa-onnx)", "Apache-2.0", "sherpa-onnx/kaldi-native-fbank"),
    ("kaldi-decoder (in sherpa-onnx)", "Apache-2.0", "sherpa-onnx/kaldi-decoder"),
    ("kaldifst and OpenFst (in sherpa-onnx)", "Apache-2.0", "sherpa-onnx/kaldifst"),
    ("KISS FFT (in sherpa-onnx)", "BSD-3-Clause", "sherpa-onnx/kissfft"),
    ("simple-sentencepiece (in sherpa-onnx)", "Apache-2.0", "sherpa-onnx/simple-sentencepiece"),
    ("piper-phonemize (in sherpa-onnx)", "MIT", "sherpa-onnx/piper-phonemize"),
    ("eSpeak NG and ucd-tools (in sherpa-onnx)", "GPL-3.0-or-later", "sherpa-onnx/espeak-ng"),
]


def native_packages() -> list[tuple[str, str, list[str]]]:
    base = ROOT / "core/data/licenses"
    return [(name, licence, licence_texts(base / folder)) for name, licence, folder in NATIVE]


def render() -> str:
    packages = sorted(
        rust_packages() + npm_packages() + font_packages() + native_packages(),
        key=lambda p: p[0].lower(),
    )
    # Keyed by the text with whitespace collapsed, so reflowed copies merge.
    groups: dict[str, tuple[str, list[str]]] = {}
    without_text = []
    for name, licence, texts in packages:
        if not texts:
            without_text.append(f"{name} ({licence})")
        for text in texts:
            key = " ".join(text.split())
            groups.setdefault(key, (text, []))[1].append(f"{name} ({licence})")

    lines = [
        (ROOT / "LICENSE-EXCEPTION.md").read_text(encoding="utf-8").strip(),
        "",
        (ROOT / "LICENSE").read_text(encoding="utf-8").strip(),
        "",
        "#" * 72,
        "",
        "Mossbook includes the following third-party software. Each component is",
        "distributed under its own licence, reproduced below.",
        "",
    ]
    for text, names in sorted(groups.values(), key=lambda g: g[1][0].lower()):
        lines += ["=" * 72, *names, "-" * 72, text, ""]
    if without_text:
        lines += [
            "=" * 72,
            "Components that ship no separate licence file; their licence is the",
            "SPDX expression in parentheses:",
            *without_text,
            "",
        ]
    return "\n".join(lines)


def main() -> int:
    text = render()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            print(f"{OUT.relative_to(ROOT)} is out of date; run tools/licenses.py")
            return 1
        return 0
    OUT.write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
