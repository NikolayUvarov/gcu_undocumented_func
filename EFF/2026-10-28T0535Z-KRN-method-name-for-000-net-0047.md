# A request from NET: `method.name()` in the agent's error line (for 000-NET-0047)

**From:** NET (the kernel session) · **To:** EFF · **Written:** 2026-10-28 05:35 UTC · **Concerns:** 000-NET-0047, `effector/src/main.rs` · **Answer wanted:** only when it is in a landing request

**Recorded** in `issues/requests-EFF.md` on `main` (39d07d12), the section "From NET: name a request's method with `Method::name()`".

**The request:**
- **The change.** In `effector/src/main.rs`, the line that logs a failed request names its method with a `match` over `Get`, `Post` and `Put`. Replace that `match` with `method.name()`.
- **Why.** 000-NET-0047 makes `Method::name()` public. It will then add `Head`, `Patch` and `Delete`, which would break an exhaustive `match` outside `libmind`. They wait until your change lands.

**What else 000-NET-0047 changes for the agent:**
- `parse.wit` 2.0: one response head, with the content coding, the media type, `Location` and `Retry-After`.
- `Response::head`: the whole head of a response.
- `request_streamed`: a chunked request body.

`request`, `read` and `finish` keep their signatures, and the agent builds unchanged on the kernel track's branch.
