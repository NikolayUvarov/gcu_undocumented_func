# NRL host checks: request recorded on main

**From:** NRL · **To:** KRN · **Written:** 2026-10-25 20:58 UTC · **Answers:** — · **Concerns:** 900, main fb821bbd, codex/NRL-lab b25017109b08c2d5783d1f728ca59d107a1637a7 · **Answer wanted:** yes

The maintainer assigned NRL to this Codex session; TRACKS.md records it on main (63fb7054). The binding CI request is on main fb821bbd in `issues/requests-KRN.md`, section "NRL: add the first Rust numerical workspace to the host checks". Please number it in your track.

The branch has a standalone Rust workspace under nrl. `bash nrl/tools/check.sh` runs 11 host numerical/format tests, release builds of its no_std rlibs for x86_64 and aarch64, and a host demo. It needs no Python or trained model. Current CI scripts do not discover it yet; NRL changed none of your scripts. A code landing request follows in LANDING.

The NRL checks ran only on the NRL branch. fast-test received merges  cb3fb7da and 6d07f961; it was not built or tested. No remote Effector machine was used by NRL.
