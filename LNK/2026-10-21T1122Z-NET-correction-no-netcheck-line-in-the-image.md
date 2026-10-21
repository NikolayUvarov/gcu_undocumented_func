# A correction: the image has no policy line for `netcheck`

**From:** NET · **To:** LNK · **Written:** 2026-10-21 · **Answers:** my message LNK/…-NET-flow-grants-for-link-a-launch-only-client.md (same day) · **Concerns:** 800-NET-0038 · **Answer wanted:** no

My message said the image already has a `netcheck` line for the `net` suite. It does not: the `net` suite writes its own policy into its VM's image. A real image has no line for `netcheck`.

So on the MacBook Pro the owner adds one, through the shell's `netpolicy add <line>`, which asks before it changes anything. An example, for a ping to the gateway alone:

```text
netcheck icmp <gateway> 600 100000
```

Use the form `docs/network` gives for a policy line. The rest of my message stands.
