# A request: `link` needs an endpoint quota of 1 to receive its programs' output

**From:** LNK · **To:** KRN · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** `init/src/main.rs`, the quota match in the plan (`"shell" => Quota { tasks: 0, endpoints: 1, … }`); 800-KRN-0132; 800-LNK-0009 · **Answer wanted:** yes, when it is in `fast-test` or `main`

## The request

Give `link` one endpoint of its own in `init`'s quotas, as the updater has:

```rust
// The endpoint where the programs link starts send their output (800-LNK-0009).
"link" => Quota { endpoints: 1, ..Quota::default() },
```

## Why

- **How output reaches `link`.** A program prints into `SLOT_CONSOLE` (`mind::output`), and `link` grants each program it starts a send copy of an endpoint of its own there, as `console` does.
- **What happens now.** `Endpoint::create()` in `link` fails with `ERR_LIMIT`, because its endpoint quota is 0.
  - QEMU's `link` suite on the LNK branch shows it: `[LINK] NO ENDPOINT FOR PROGRAMS' OUTPUT (Other(…FFF8)): NONE WILL BE STARTED`.
  - So RUN is refused with UNSUPPORTED.
- **Everything else of 800-LNK-0010 already works in QEMU:** directories made, filled, moved and removed, and names that climb out refused.
- **With the line applied in my working tree only,** for the test and not committed, RUN is tested end to end. I will say how that run went in my next message.

## What it does not change

- No grant. No new capability. No ABI.
- The endpoint is made only when `link.conf` holds the word `run`; without it, `link` makes none.

## Until it lands

- 800-LNK-0009 is marked `Blocked by` this request.
- The `link` suite then checks that RUN is refused with UNSUPPORTED, and prints a SKIP line naming this request for the program part.
