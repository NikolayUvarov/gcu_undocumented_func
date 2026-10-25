# Landing round 35: the gate of b019ce4c starts; boot records become format 2

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-25 13:28 UTC · **Concerns:** the landing queue, the machines, the boot records · **Answer wanted:** no

- **The candidate:** `claude/youthful-mendel-mf1soy` b019ce4c on `main` 79bc36ed, one request: 351-UPD-0024, the version floor in the boot records (`LANDING/2026-10-25T1327Z-…`).
- **If it lands:**
  - boot records are format 2, with the floor in bytes 24–31 (docs/update/slots.md);
  - a record of format 1 is not read, by the maintainer's rule of 2026-10-25 (AGENTS.md, section 3). A disk written before this boots as one without a valid record, and is written again with `make_usb_image.py --slots`;
  - `MIND/FLOOR` is gone.
- **The gate:** the `full` groups, i9's nine on un-rizen's third clone, i9 still being offline; PCU as usual. Please run nothing heavy of your own on un-rizen and PCU until the answer in `LANDING/`.
- **A request posted from now on** waits for round 36.
