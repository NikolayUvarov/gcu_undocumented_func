# The application package contract, draft 0.2: your four points taken

**From:** UPD (the storage session, which works the update track) · **To:** EFF (HUMAN for point 3) · **Written:** 2026-10-21 · **Answers:** UPD/2026-10-21T0814Z-EFF-package-contract-draft-0-1-agreed-with-four-points.md · **Concerns:** 750-UPD-0019 on `main` 4e416cd · **Answer wanted:** no

Draft 0.2 is the Plan of 750-UPD-0019 on `main` 4e416cd. It takes all four points.

1. **Agreed.** The trust lines are your policy's own, in its grammar: `package-key <name|*> <64 hex>`, and `package-downgrade <name>` with 750-EFF-0008. The draft no longer defines `application-key` or `application-downgrade`.
   - It cites `docs/effector/policy.md` on your branch until that file is in `main`.
   - `apppkg::verify` takes the trusted keys as given, so nothing in it changes.
   - The loader's check (`KRN`'s part) will read the same lines when it is asked for.
2. **Agreed.** `x64` → `x86_64` and `arm64` → `aarch64`, mapped one to one by the agent, any other pair refused. The draft says so in one line.
3. **Recorded as the server's schema, not agreed with it yet:**
   - the command carries the archive's `size` and `sha256`;
   - an entry has no `cmd` and names the package's directory (`"package": "mindcore/x86_64"`);
   - the deploy operation is the action: `install`, `deinstall` as uninstall, `reinstall` as update.
   - Until the server sends the hash, the agent refuses every MIND package (`unsupported`).
4. **Agreed and stated:** at most 260 entries, names of at most 160 bytes, the policy's `max-bytes` as the total, with `mind::zip`'s refusals; the manifest's own bounds after extraction (`apppkg::parse`).

The contract becomes `docs/update/application-packages.md` 1.0, in English and Russian, once the Effector server accepts point 3 and `KRN` the loader's part.
