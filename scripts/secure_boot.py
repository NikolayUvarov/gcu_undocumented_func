#!/usr/bin/env python3
"""Secure Boot with our own keys (351-UPD-0012, docs/update/secure-boot.md; MC-9.1, 9.4, 9.6).

The firmware's own check is what makes the bootloader's checks count: with our PK, KEK and db enrolled, the machine
runs only a bootloader signed with our db key, and not one whose hash is in dbx.

Usage:
    secure_boot.py keys DIR                           a test PK, KEK and db (RSA-2048, SHA-256) in DIR, labelled TEST
    secure_boot.py sign EFI DIR OUT                   OUT: EFI signed with DIR's db key (sbsign)
    secure_boot.py hash EFI                           the Authenticode SHA-256 of a loader, as dbx lists it
    secure_boot.py vars TEMPLATE DIR OUT [--revoke EFI]...
                                                      an OVMF variable store: DIR's PK, KEK and db enrolled, Secure Boot
                                                      on, and each revoked loader's hash in dbx

Needs openssl, sbsign (sbsigntool) and the virt-firmware Python package (python3-virt-firmware). Real keys are made
the same way on a machine kept apart, and never enter the repository; the boot key and the release key are others.
"""
import logging
import subprocess
import sys
import warnings
from pathlib import Path

ROLES = ("PK", "KEK", "db")
# The owner GUID our entries carry in the signature lists.
OWNER = "5f0a2c3e-6d1b-4b8e-9c47-6a1d2e3f4b5c"


def keys(directory, label="MIND Core TEST"):
    """Self-signed RSA-2048 certificates for PK, KEK and db, each with its private key."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    for role in ROLES:
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-nodes", "-days", "3650",
                        "-subj", f"/CN={label} {role}", "-keyout", str(directory / f"{role}.key"), "-out", str(directory / f"{role}.crt")],
                       check=True, capture_output=True)
    return directory


def sign(efi, directory, out):
    """OUT: the loader signed with the db key of `directory`; OUT may be the loader itself."""
    directory, out = Path(directory), Path(out)
    signed = out.with_name(out.name + ".signed")
    subprocess.run(["sbsign", "--key", str(directory / "db.key"), "--cert", str(directory / "db.crt"), "--output", str(signed), str(efi)],
                   check=True, capture_output=True)
    signed.replace(out)
    return out


def authenticode(efi):
    """The Authenticode SHA-256 of a PE image (the hash dbx and db list), with any signature left out."""
    import pefile
    from virt.peutils import pesign
    # pesign logs a section with no file data (the loader's .data) as unexpected; it adds nothing to the hash.
    logging.disable(logging.ERROR)
    try:
        digest = pesign.pe_authenticode_hash(pefile.PE(str(efi)))
    finally:
        logging.disable(logging.NOTSET)
    return digest.hex() if isinstance(digest, bytes) else digest


def variables(template, directory, out, revoke=()):
    # virt-firmware reads certificate dates through a property cryptography has deprecated: noise, not a fault.
    warnings.filterwarnings("ignore", message="Properties that return a na")
    from virt.firmware.varstore import autodetect
    directory = Path(directory)
    store = autodetect.open_varstore(str(template))
    if store is None:
        raise ValueError(f"{template}: not a variable store this tool knows")
    varlist = store.get_varlist()
    varlist.add_cert("PK", OWNER, str(directory / "PK.crt"), replace=True)
    varlist.add_cert("KEK", OWNER, str(directory / "KEK.crt"), replace=True)
    varlist.add_cert("db", OWNER, str(directory / "db.crt"), replace=True)
    for efi in revoke:
        varlist.add_hash("dbx", OWNER, authenticode(efi))
    varlist.enable_secureboot()
    store.write_varstore(str(out), varlist)
    return Path(out)


def main(argv):
    if argv[:1] == ["keys"] and len(argv) == 2:
        print(f"TEST KEYS IN {keys(argv[1])}: {', '.join(ROLES)}")
    elif argv[:1] == ["sign"] and len(argv) == 4:
        print(f"SIGNED {sign(argv[1], argv[2], argv[3])}")
    elif argv[:1] == ["hash"] and len(argv) == 2:
        print(authenticode(argv[1]))
    elif argv[:1] == ["vars"] and len(argv) >= 4:
        revoke = [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == "--revoke"]
        print(f"VARIABLES {variables(argv[1], argv[2], argv[3], revoke)}: PK, KEK, db enrolled, Secure Boot on, {len(revoke)} revoked")
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
