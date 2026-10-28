#!/usr/bin/env python3
"""The public root store shipped as tlsroots.pem (000-NET-0046; MC-11.6, MC-12.1).

Mozilla's roots for TLS server authentication, as curl extracts them from Mozilla's NSS (https://curl.se/docs/caextract.html),
are committed in third_party/roots/cacert.pem with their record in third_party/roots/SOURCE.md, so a build ships the same
bytes every time. The image builders copy the store to the boot volume's root as tlsroots.pem; `tls` reads it at each
connection, and the owner may replace or trim it there.

Usage: update_roots.py          fetch the current store, check its SHA-256 against curl's and that every certificate
                                parses, show the roots added and removed, and write the store and its record
       update_roots.py --check  check the committed store against its record (the builders run this)
"""
import base64
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STORE = ROOT / "third_party" / "roots" / "cacert.pem"
RECORD = ROOT / "third_party" / "roots" / "SOURCE.md"
URL = "https://curl.se/ca/cacert.pem"
DIGEST_URL = URL + ".sha256"
# tls reads at most this much of tlsroots.pem (tls/src/main.rs, roots()).
TLS_READ_MAX = 512 * 1024
BLOCK = re.compile(r"(?:^|\n)([^\n]*)\n=+\n(?:# Fingerprint \(SHA-256\): ([0-9A-F:]+)\n)?-----BEGIN CERTIFICATE-----\n([A-Za-z0-9+/=\n]+?)-----END CERTIFICATE-----")


def roots(text):
    """The store's roots as (name, DER), in order; raises ValueError on a block that is not a DER certificate."""
    found = []
    for name, fingerprint, body in BLOCK.findall(text):
        der = base64.b64decode(body, validate=False)
        if fingerprint and fingerprint.replace(":", "").lower() != hashlib.sha256(der).hexdigest():
            raise ValueError(f"{name!r}: its SHA-256 is not the fingerprint above it")
        # A certificate is one DER SEQUENCE with a long-form length that covers the rest exactly.
        if len(der) < 4 or der[0] != 0x30 or der[1] not in (0x81, 0x82):
            raise ValueError(f"{name!r}: not a DER certificate")
        length = der[2] if der[1] == 0x81 else der[2] << 8 | der[3]
        if 2 + (der[1] & 0x7F) + length != len(der):
            raise ValueError(f"{name!r}: its length is not the certificate's")
        found.append((name.strip(), der))
    if text.count("-----BEGIN CERTIFICATE-----") != len(found):
        raise ValueError("a certificate block without its name line")
    if not found:
        raise ValueError("no certificates")
    return found


def mozilla_date(text):
    match = re.search(r"Certificate data from Mozilla as of: ([^\n]+)", text)
    if not match:
        raise ValueError("no 'Certificate data from Mozilla as of' line")
    return match.group(1).strip()


def record_text(data, text):
    digest = hashlib.sha256(data).hexdigest()
    return f"""# The public root store (000-NET-0046)

`cacert.pem` holds Mozilla's root certificates for TLS server authentication, as curl extracts them from Mozilla's NSS. The image builders ship it as `tlsroots.pem` at the boot volume's root. `scripts/update_roots.py` fetches a newer one, and `--check` checks this file against the record below.

- **Source:** {URL} (https://curl.se/docs/caextract.html)
- **Mozilla's data as of:** {mozilla_date(text)}
- **Roots:** {len(roots(text))}
- **Bytes:** {len(data)}
- **SHA-256:** `{digest}`
- **Licence:** MPL-2.0 (`LICENSES/MPL-2.0.txt`): curl's page says the file is "licensed under the same license as the Mozilla source file: MPL 2.0".
"""


def check():
    data = STORE.read_bytes()
    text = data.decode("utf-8")
    recorded = re.search(r"\*\*SHA-256:\*\* `([0-9a-f]{64})`", RECORD.read_text())
    if not recorded or recorded.group(1) != hashlib.sha256(data).hexdigest():
        raise SystemExit(f"{STORE}: its SHA-256 is not the one in {RECORD}; run scripts/update_roots.py")
    if len(data) > TLS_READ_MAX:
        raise SystemExit(f"{STORE}: {len(data)} bytes, more than the {TLS_READ_MAX} tls reads")
    count = len(roots(text))
    print(f"tlsroots.pem: {count} roots, Mozilla's data as of {mozilla_date(text)}")


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def update():
    data = fetch(URL)
    expected = fetch(DIGEST_URL).decode().split()[0].lower()
    if hashlib.sha256(data).hexdigest() != expected:
        raise SystemExit(f"{URL}: its SHA-256 is not the one curl publishes at {DIGEST_URL}")
    if len(data) > TLS_READ_MAX:
        raise SystemExit(f"{URL}: {len(data)} bytes, more than the {TLS_READ_MAX} tls reads")
    text = data.decode("utf-8")
    new = roots(text)
    old = roots(STORE.read_text()) if STORE.exists() else []
    old_der, new_der = {d for _, d in old}, {d for _, d in new}
    for name, der in new:
        if der not in old_der:
            print(f"+ {name}")
    for name, der in old:
        if der not in new_der:
            print(f"- {name}")
    STORE.parent.mkdir(parents=True, exist_ok=True)
    STORE.write_bytes(data)
    RECORD.write_text(record_text(data, text))
    print(f"{STORE.relative_to(ROOT)}: {len(new)} roots, Mozilla's data as of {mozilla_date(text)}")


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        check()
    elif not sys.argv[1:]:
        update()
    else:
        raise SystemExit(__doc__)
