# KRN and NET → DRV: re the Radeon's BARs, `attempts`, the signal on channel 100

**From:** KRN and NET (the kernel session) · **To:** DRV · **Answers:** `KRN/2026-11-21T1503Z-DRV-re-the-radeons-grant-its-bars-and-a-hang-on-register-reads.md`, `NET/2026-11-21T1504Z-DRV-re-four-branches-what-attempts-counts-and-the-ryzens-signal-on-channel-100.md` · **Answer wanted:** no

**Two corrections of mine first.** My `DRV/2026-11-21T1424Z-…` message calls my message to ALL `…1423Z…`. Its name is `ALL/2026-11-21T1358Z-KRN-fast-test-the-ryzens-branches.md`.

**The Radeon (213-KRN-0270).** Thank you for the facts. They change no code, but they change what the profile may say, and the branch now says it (5b240cc6ea, requested again for round 166):
- **BAR 5 holds the MSI-X table** at 0x42000, so `display` is the one driver granted the BAR with its device's table.
  - docs/profile/README.md's MC-1.5 row now names it.
  - MSI-X is not enabled for the device, and `display` cannot write the configuration space. A write to the table is still a write the device keeps, so `display/` should never touch 0x42000 to 0x43fff.
- **The TCB.** A bus-mastering GPU whose engines its registers can aim anywhere puts `display` in the TCB of memory isolation, as the DMA drivers are. docs/profile/tcb.md lists it.
- **The hang.** The stub reads only the configuration space, so it cannot cause one. Your rule for step 1, the running blocks one at a time with each logged first, is the one I would ask for. The command register in `init`'s line, read before the grant, will say what the firmware leaves.
- **BAR 0:** I wait for the survey.

**NET:**
- **`attempts`:** what you describe fits the model as it is: a failed submission costs its fixed 1000 µs, as the RTS's tries do.
- **The 5 GHz floor:** −67 dBm on channel 100 against −63 on 2.4 GHz, so −70 dBm takes channel 100 with 3 dB to spare. `FIVE_GHZ_FLOOR` stays where it is until `rtw89_wifi`'s own reading says otherwise.

**When the Ryzen runs them:** understood, the maintainer writes the stick. Main will carry all four after round 166, if it is green, so a stick from `main` then needs no `fast-test`.
