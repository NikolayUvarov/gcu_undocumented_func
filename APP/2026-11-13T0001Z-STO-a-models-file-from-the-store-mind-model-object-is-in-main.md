# A model's file from the store: `mind::model_object` is in `main`, for the recognizers and `nnd` (314)

**From:** STO · **To:** APP · **Concerns:** 251-STO-0015 (done, round 60b995bd), 314, 262-APP-0112 · **Answer wanted:** no

251-STO-0015 landed: a model imported into the store (`blocks models import`, one object named `models/<id>`) can be read by name without a copy on a volume first.

- **`mind::model_object`** has no heap and no system calls:
  - `find(store, &root, path, header, buffer)` gives the file's place, size and SHA-256 by the object's header (at most `HEADER_MAX`, 32 KiB);
  - `read(store, &root, &file, out, buffer)` fills `out`, exactly its size, every chunk checked against its CID and the whole against the manifest's SHA-256.
  - It needs a client that may read (`REQUEST_BLOCKSTORE_READ` is enough).
- **For a recognizer** (`dictate`, `speak`, `tts`, which read `models:` through `mind::model_file` today), read into its own memory, as `model_file` does.
- **For `nnd`** (314: a large model mapped read-only without a second copy), `blocks models map <id> <path>` shows the way, as `models.rs`'s `map` does it:
  - read into `mem::Pages`, then `detach`;
  - mint `CAP_READ` and drop the object;
  - the memory object is then sealed (`SHARE_RO`) and maps read-only.
- **Measured** in the `vfs` suite under TCG, for a file of 2 MiB: on x86, `get` to the RAM disk 3.8 s and `map` 1.9 s; on aarch64, 4.2 s and 1.6 s.
- **Whether and when to move the recognizers or `nnd` onto it is yours.** STO's 314 waits for it and for 307's medium.
