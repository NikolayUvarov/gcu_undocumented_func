# The protocol's test vectors (100003-APP-0145, 0148)

What a hub or a client of `hub-api 2` must give, in any language. `tests/proto.rs` makes each file and compares it; `IMPROVE_PROTO_WRITE_VECTORS=1 cargo test` writes them anew after a change the design allows.

- **`signature.txt`:**
  - the key is Ed25519 from a seed of 32 bytes `0x2a`;
  - the request: its method, path, body and the hub's challenge;
  - `signed` is the signed bytes in hex: `MIND Hub request 1\0`, the method, the path, the body's SHA-256 in hex and the challenge in hex, a line end after each but the last;
  - the `Hub-Signature` header's value, and the device ID, the SHA-256 of the public key.
- **`states.txt`:** every move allowed, as `role from to`; every other move is refused.
- **`trust.txt`:** `trust event result` for the hub's own changes, within −256 … +512.
- **`bucket.txt`:** a bucket of size 3 and 4 a day, a request every 2 hours: `time result tokens-left`.
  - `taken`: a token was there;
  - `wait N`: N ms until the next one.
- **`repeat.txt`:** the repeat digests of two requests: `Fan  NOISE` / `idle\n\nloud`, and `Тише` / `вентилятор`.
- **`work.txt`:** the proof of work's first counter for a challenge of 32 bytes `0x03` at 12 bits.
- **`request.json`:** a request in the API's one encoding.
- **`message.json`:** a verdict with an attachment's entry, in the same encoding.
- **`hub-api 1`** is refused by its name: its request had no `named` and its message no attachments (100003-APP-0148).
