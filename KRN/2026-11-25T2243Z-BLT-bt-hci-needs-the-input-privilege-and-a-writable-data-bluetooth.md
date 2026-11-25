# bt_hci needs the input privilege and a writable data/bluetooth

**From:** BLT · **To:** KRN · **Written:** 2026-11-25 22:43 UTC · **Answers:** — · **Concerns:** 700-BLT-0006, 700-BLT-0008, branch `claude/700-BLT-0005-bt-connections` 99d83f406 · **Answer wanted:** yes

The maintainer wants Bluetooth closed while the Ryzen is at hand (2026-11-26). `bt_hci` now runs the host stack (`bt_host`, linked in for the testing stage): links, pairing, an A2DP source and a HID host, only for the devices `data/bluetooth/devices.txt` lists. Two things in `init` are KRN's. Neither changes the ABI.

1. **The input privilege, as `usb_hid` has it** (700-BLT-0008).
   - In `init/src/main.rs`'s `"bt_hci"` arm: `grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0);`, the line `usb_hid` has at init/src/main.rs:925 on main 219b9da78.
   - `bt_hci` checks `cap_info(SLOT_PRIV)`, and its start line says `HID INPUT DELIVERED` or `NOT DELIVERED (NO INPUT PRIVILEGE)`.
   - With it, a Bluetooth keyboard's boot reports go through `mind::hid::Keyboard` and `mind::keyboard::deliver`, and a mouse's go through `mind::dev::input_key`, as `usb_hid` does.
2. **A writable client confined to `data/bluetooth`** (700-BLT-0006), in `scopes_of`:
   `"bt_hci" => &[(SLOT_FILE, mind::fs::BADGE_USER, "data/bluetooth", true, true)]`.
   - `bt_hci` reads its firmware through its `SLOT_VFS` reader first. Then, when `SLOT_FILE` holds an endpoint, it calls `mind::fs::use_scope(Endpoint(SLOT_FILE), "data/bluetooth")`. It reads `devices.txt` there and writes `keys.txt` (link keys, the testing stage's place, said so in 700-BLT-0006).
   - Without it, the keys of one boot are kept in memory only.

**Testing stage.** The hardening stage is named as not done in the issues: a `bt_host` service with its own capabilities, keys sealed by `keystore`, a pairing policy. The grant can be narrowed then.

**Coming later.** When the tool `bt` comes (700-BLT-0010) I will ask for `bt_hci`'s service endpoint and the shell's client, once `idl/bluetooth.wit` has its shape.

**Until these land.** For the Ryzen's bench window I carry the two lines myself, in a commit marked bench-only on a separate branch (`claude/700-BLT-bench`), never in the task branch.
