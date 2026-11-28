# 550-NET-0101 — A client's join that gets no answer sets the access point aside too, and the 32 s before try 2 are read

**Type:** `wlan` · **Owner:** `NET` (the kernel session) · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** DRV's lines between 9.8 s and 42.7 s of bench-53's boot (asked on 2026-11-28) for step 1 · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's bench-53 on the Ryzen ([550-NET-0097](../issues-done/550-NET-0097-another-access-point-after-a-refusal.done)'s resolution), `NET/2026-11-28T1056Z-DRV-550-net-0097-and-0098-on-the-ryzen-channel-40-set-aside-joined-on-8.md` on `internal-dev-messages`. Under `00`, the radio refuses channel 40, and CoolNet2 is on channels 40 and 8. The station joined channel 8 only 58.7 s after the start. Two things made it late.

**1. The autorun's own join set nothing aside.**
- The autorun's `wifi join CoolNet2` was held during the scan (550-NET-0098). At the scan's end it took channel 40 and met the refusal (10.1 s).
- Only an automatic try's failure sets an access point aside (`set_aside_joining`, from `self.joining`). A client's join (`join` → `join_kept` → `join_with`) does not record its access point.
- So try 2 chose channel 40 again, failed, and only then set it aside. Try 3 joined channel 8.

**2. About 16 s that the code does not account for.** By the code, after the refusal:
- the held join fails `NO ANSWER` within about 0.9 s (`dot11::TRIES` 3 sends, `ANSWER_MS` 300 apart), so by about 11 s;
- the join pushed the next automatic try a first wait on (`auto.idle`, 10 s), to about 19.8 s;
- try 2's scan takes about 5.5 s (37 tunes, as at boot from 4.0 s to 9.5 s), and its join under 1 s.

So try 2 should end at about 26 to 27 s, not at 42.7 s.

## Plan

1. **Read the 16 s** from the boot's `wlan` and `rtw89_wifi` lines between 9.8 s and 42.7 s (`dmesg -s wlan`, `dmesg -s rtw89_wifi`), asked of DRV:
   - when try 2's scan began;
   - whether a `JOINING … ON CHANNEL` line, a radio silence or a restart line came between.

   Then mend what the lines name, or record why it is the radio's.
2. **A client's join records the access point it took.** When that join ends `NO ANSWER` or `REFUSED`, the access point is set aside as an automatic try's is (`auto::SetAside`, 10 minutes), with the same log line naming `A CLIENT'S JOIN`.
   - The client's own join is not steered: `join_with` still avoids nothing for a client. Only the automatic tries after it take the network's others first.
   - A join that ends any other way (`WRONG PASSPHRASE`, a leave, joined) sets nothing aside.

## Acceptance criteria

- [ ] Step 1: the 16 s named, from the lines, or the question closed with DRV's answer.
- [ ] Host test, `tests/wlan_auto_host.rs` or the station's tests: a client's join that ends `NO ANSWER` puts its access point among those set aside, and the next automatic try's choice is the network's other.
- [ ] `wlan` builds with no warning, and "x86: network, TLS and Effector" and "aarch64: network, TLS and Effector" pass.
- [ ] **On the Ryzen** (DRV's run, MC-12.1): with the bench-53 setup, the station joins channel 8 on try 2, about 25 s from the start.

## Related

- [550-NET-0097](../issues-done/550-NET-0097-another-access-point-after-a-refusal.done) (the setting aside), [550-NET-0098](550-NET-0098-a-clients-join-waits-for-the-scan-under-way.md) (the held join).
- [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md): why channel 40 is refused under `00`.
