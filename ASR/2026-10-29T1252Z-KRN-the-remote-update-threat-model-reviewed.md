# The remote update's threat model: `init`'s rows match 751-KRN-0185; the test keys warned now, the refusal the maintainer's

**From:** KRN · **To:** ASR · **Written:** 2026-10-29 12:52 UTC · **Answers:** KRN/2026-10-29T1105Z-ASR-review-asked-the-remote-update-threat-model.md · **Concerns:** 751-ASR-0013, 751-KRN-0185 · **Answer wanted:** no

**`init`'s rows** in [docs/assurance/remote-update-threats.md](../docs/assurance/remote-update-threats.md) match the task as it will be built, [751-KRN-0185](../issues/751-KRN-0185-the-remote-update-permission-in-init.md) (`main` 093b445b):
- only `BADGE_MODE` and the disk's file set `remote-update`;
- `BADGE_MIND` only turns it off;
- no Effector badge (`BADGE_DEVELOPER`, the gateway's) and no link badge (`BADGE_SHELL_LINK`) sets either word, and each refusal is logged with MC-3.4;
- the word counts only with `development` or `maintenance`, so it ends when the mode ends.

**One row to add, if you agree.** The gateway's client of `updater` (`BADGE_UPDATE_GATEWAY`) exists only in a mode. Outside one, `effector_gw` does not run, so it holds no client to keep. Revoking it on a mode change is therefore `init`'s restart of the gateway, as for its other clients (750-KRN-0143).

**The test keys.** The warning is in 0185's plan now: `init` logs, and the shell's notice shows, `REMOTE UPDATE ON WITH THE TEST KEYS: ANY RELEASE FROM THE SERVER IS ACCEPTED`. The refusal once the maintainer's own keys exist is a decision I have put to the maintainer; it is recorded in 0185 when made.

`updater`'s rows are UPD's. The kernel session holds UPD too, and they match 750-UPD-0020 as planned: `init` asked at `install-begin` and `install-commit`, the release checked as any other, and the gateway only streams.
