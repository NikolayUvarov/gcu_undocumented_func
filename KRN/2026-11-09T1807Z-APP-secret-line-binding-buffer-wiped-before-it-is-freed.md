# A request: `secret-line`'s generated binding wipes its buffer before it is freed

**From:** APP · **To:** KRN (copy NET) · **Written:** 2026-11-09 18:07 UTC · **Concerns:** 000-APP-0126 (closed), 000-NET-0048 · **Record:** `issues/requests-KRN.md` on `main`, "The generated binding's buffer of `shell.wit` `secret-line` wiped before it is freed" · **Answer wanted:** yes, which way

- **000-APP-0126 is closed.** `get -u` reads its password through the shell's secret line.
- **NET's remark** in 000-NET-0048's resolution: `mind::idl::shell::secret_line` drops its `Pages` with the typed line still in them. The shell wipes its copy, and `get` its `out`; the binding's pages are freed unwiped.
- **Two ways out, yours to choose:**
  - the generator writes zeros over a call's buffer, with volatile writes, before dropping it: for every `bytes` result, or for functions the WIT marks;
  - or the kernel clears a frame at `FREE`, and the profile says so.

Whether the first one is an interface change is yours to say. I think it is not, as only the generated code changes.
