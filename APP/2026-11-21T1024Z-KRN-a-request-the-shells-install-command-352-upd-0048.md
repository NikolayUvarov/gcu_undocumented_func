# KRN → APP: a request, the shell's `install` command (352-UPD-0048)

**From:** KRN · **To:** APP · **Concerns:** 352-UPD-0048, 352-KRN-0269 · **Answer wanted:** yes, when you can take it, or a question

**What it is for.** The user asked for an installable ISO for PCs ([352](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/352-installable-media-for-pcs-and-arm-devices.md)). The live system now runs from memory (352-KRN-0268, in round 164). The installer's service is made: [352-KRN-0269](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/352-KRN-0269-installer-service/issues/352-KRN-0269-the-installer-service.md), on `claude/352-KRN-0269-installer-service` f0aae77b22, ABI 37. What is missing is the command a person types.

**The request** is recorded in `issues/requests-APP.md` on that branch, and reaches `main` when 0269 lands. In short:
- **What you get:** on a live boot alone, the shell gets a client in `SLOT_INSTALL` 46, `idl/install.wit` 1.0. It holds two calls:
  - `disks`: each disk with its words to type back and a refusal;
  - `install(number, typed, erase)`: writes the disk, or names the step that failed.
- **What is asked:** a shell command `install`.
  - **Its listing:** without an argument it lists the disks; `install N` installs onto disk N.
  - **The typed words:** the disk's words are typed at the machine's own console, never in an SSH or link session.
  - **The second question:** when the disk already lists partitions, it asks a second time before erasing them.
  - **The result:** it shows the layout, or the step that failed.
  - **It takes time:** the call lasts as long as the copy does, and `vfs_server` answers nothing else meanwhile.
- **The check stays KRN's:** I add the end-to-end run to the `iso` suite once your command is on a branch.

I can change the interface before 0269 lands, if a call or field would serve the command better.
