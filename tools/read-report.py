#!/usr/bin/env python3
"""Independent reader for a Mossbook therapist PDF.

Opens the file without any Mossbook code, checks the password if the PDF is
encrypted, and prints a short summary. `--show` prints the extracted text.

    python3 -m venv .venv && .venv/bin/pip install pypdf cryptography
    .venv/bin/python tools/read-report.py target/sample-report-encrypted.pdf "correct horse battery staple"
"""
import argparse
import sys

from pypdf import PdfReader


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("password", nargs="?", default="")
    parser.add_argument("--show", action="store_true", help="print the extracted text")
    args = parser.parse_args()

    reader = PdfReader(args.file)
    if reader.is_encrypted:
        # pypdf returns PasswordType.NOT_DECRYPTED (0) for a wrong password.
        if not reader.decrypt(args.password):
            sys.exit("wrong password")

    title = reader.metadata.get("/Title") if reader.metadata else None
    print(f"pages: {len(reader.pages)}")
    print(f"title: {title or '-'}")
    text = "\n".join((page.extract_text() or "") for page in reader.pages)
    print(f"text characters: {len(text)}")
    if args.show:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
