# KRN landing request: HTTP's HEAD, PATCH and DELETE; the shell's audio badge; link's input in dev; the hold's note kept

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 13:08 UTC

**Ready:** `claude/youthful-mendel-mf1soy` a64efe8e2a05c1d1752e3f56ccf63ff7da9c36ca

## What it brings (`main` 57c2fd3d and its text commits merged)

- **000-NET-0047** (advances; its last part):
  - `mind::http::Method` gains HEAD, PATCH and DELETE, and `Method::parse`;
  - the answer to HEAD ends at its head, and a 303 keeps HEAD;
  - `get -I` sends HEAD, and `-X` takes the three;
  - the stand-in answers them, and `webcheck --api` checks them on one connection;
  - `--suites download` runs the net suite's download checks alone.
- **000-KRN-0118** (advances; P1 on the maintainer's report of a quiet microphone): `init` gives the shell's `audio_gw` client `mind::audio::BADGE_LEVELS`; programs keep the loader's plain one. `audio_gw` 1.1 reads no badge.
- **000-KRN-0186's note** (783ad529): a task held a whole second for the compositor's copy is noted in the kernel's ring only, no longer on the serial line. It cut into a typed command's echo and failed round 52.
- **800-KRN-0191** (LNK's request, for 800-LNK-0032):
  - with `dev` in `link.conf`, `link` holds the input privilege in slot 8, never in a safe start;
  - `init`'s developer-mode line names it.

## Tested

- **Host:** `http` (21 tests), `get` and `sse`, `multipart` and `http_coding`.
- **x86, QEMU under TCG, locally:**
  - `--suites download` whole (14 checks);
  - `tls` (6);
  - `services` (5), with the shell's slots 2 and 4 badged and the loader's plain;
  - `link`, with slot 8 and `init`'s line.
- **aarch64, on the machines:**
  - `network, TLS and Effector` (rz-2, 859 s);
  - `programs, shell and four CPUs` (rz, 438 s);
  - `RAM, ACPI and PCI above 4 GiB` with the hold's fix (acc2, 325 s, where round 52 failed).
- **Builds:** x86 and aarch64.
- **`fast-test`:** 8cbef246.
