# Review asked: the remote update's threat model (751-ASR-0013), and one decision about the test keys

**From:** ASR · **To:** KRN, for KRN and UPD · **Written:** 2026-10-29 11:05 UTC · **Concerns:** 751, 751-ASR-0013, the requests of 0956Z in `KRN/` and `UPD/` · **Answer wanted:** yes, remarks on the table, and the decision below

The model is [docs/assurance/remote-update-threats.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/docs/assurance/remote-update-threats.md) (`main` dbfc1141).
- **What it covers:** each attack on 751's path, with the check asked of your parts and the test asked in 751-ASR-0013.
- **Status:** a design until the parts exist; nothing in it is claimed (MC-12.3).

## Please check that the table names your parts as you mean to build them

- **`init`** (KRN):
  - only `BADGE_MODE` and the disk set `remote-update`;
  - `BADGE_MIND` only turns it off;
  - no Effector badge and no link badge sets it.
- **`updater`** (UPD):
  - asks `init` at `install-begin` and again at `install-commit`;
  - the release is checked as any other, the floor included;
  - the gateway holds no file scope `updater` reads, and only streams.

## The decision asked (KRN and the maintainer)

**The test keys.** With `remote-update` on, whoever makes the Effector server send a command installs any release the machine's keys accept. The DEXP and the MacBook Pro trust the public test keys, so on them the path installs anyone's code.

The model calls this acceptable on the maintainer's development machines only, while active development lasts. It leaves open whether `init` should refuse the word, or warn loudly, on a disk whose `updater` trusts the test release key. The development disks need both the word and the test keys today, so refusing outright would block 751's purpose there.

Two proposals:
- **A warning:** `init` logs, and the shell's notice shows, `REMOTE UPDATE ON WITH THE TEST KEYS: ANY RELEASE FROM THE SERVER IS ACCEPTED`.
- **A refusal later:** once the maintainer's own keys exist, the word is refused on a disk that trusts the test keys.
