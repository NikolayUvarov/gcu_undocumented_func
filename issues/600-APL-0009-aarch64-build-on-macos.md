# 600-APL-0009 — The aarch64 build with the Bash and sed macOS ships

**Type:** build scripts · **Owner:** `APL` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — for the change; the check on macOS needs a Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1 (the build host is part of the configuration a result is stated for)

Part of main task [600](600-apple-silicon-mac-vm-host.md).

## Problem

`ARCH=aarch64 ./02_build.sh` starts `scripts/build_aarch64.sh` through its first line, `#!/bin/bash`. On macOS that is Bash 3.2, and the script (expected; not yet run on a Mac):

- reads the list of programs with `mapfile`, which Bash 3.2 does not have, so it stops at once;
- takes `USER_CRATES` from `02_build.sh` with `sed -n '/^USER_CRATES=(/,/^)/{s/^ *"\([^"]*\)"$/\1/p}'`; macOS's sed is expected to reject the `}` right after the `p` flag, so the list would be empty and the script would stop with `USER_CRATES not found in 02_build.sh`;
- expands an array that may be empty (`"${features[@]}"`) under `set -u`, an error in Bash before 4.4.

`04_make_usb_image_aarch64.sh` runs `02_build.sh` and stops the same way. The guide's workaround is to run the script with Homebrew's Bash and GNU sed first in `PATH`.

## Plan

- Read the list with a `while read` loop; write the sed program in the form POSIX sed accepts (`;` before `}`); expand the features array in a form Bash 3.2 accepts.
- Linux unchanged: the same files in `aarch64_root/` (CI's aarch64 jobs build them).
- The scripts belong to the porting work of issue 204: only these lines change, and `PRT` reviews the change.
- `bash -n` here; the real check on a Mac with `/bin/bash` and `/usr/bin/sed`.

## Acceptance criteria

On an Apple Silicon Mac with macOS's own Bash and sed, `ARCH=aarch64 ./02_build.sh`, with and without `--fixtures`, and `./04_make_usb_image_aarch64.sh` build the same set of files as on Linux. The Linux CI build is unchanged. The guide drops the workaround, in both languages.

## Related

[600](600-apple-silicon-mac-vm-host.md), [600-APL-0010](600-APL-0010-run-script-on-macos.md), [docs/apple-silicon.md](../docs/apple-silicon.md) (section 2), [204](../issues-done/204-aarch64-profile-and-ci.done).
