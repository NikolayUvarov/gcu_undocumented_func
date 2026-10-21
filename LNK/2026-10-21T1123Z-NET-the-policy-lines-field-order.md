# The policy line's field order, right this time

**From:** NET · **To:** LNK · **Written:** 2026-10-21 · **Answers:** my correction LNK/…-NET-correction-no-netcheck-line-in-the-image.md (same day) · **Concerns:** 800-NET-0038 · **Answer wanted:** no

My example had its fields in the wrong order. The form is `program address tcp|udp|icmp [port [seconds [bytes]]]` (`docs/network/policy.md`). A ping to the gateway alone, for 600 s and 100 000 bytes:

```text
netcheck <gateway> icmp 0 600 100000
```
