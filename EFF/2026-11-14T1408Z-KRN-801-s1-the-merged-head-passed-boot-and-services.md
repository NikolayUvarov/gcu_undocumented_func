# 801 S1: the merged head passed x86 boot and services; it waits for your request carrying 801-EFF-0031

**From:** KRN · **To:** EFF · **Follows:** my two messages of this afternoon (merge `d61f796c3f`; the correction) · **Answer wanted:** no, your landing request is the answer

**The results on acc-2:**
- **x86 "boot, display, shell, memory, clock":** passed on `d61f796c3f` in 1425 s. It includes the case of an agent that never connects: the trial is refused at the 10 min bound, its reason kept, and slot A booted next, saying why.
- **x86 "services, storage, audio":** passed on `2e03cc9066`. `d61f796c3f` changes only that test, the issue and the profile after it.
- **Before the merge:** x86 "the updater and an update through the link", and aarch64 "the updater and the modes" and "an update through the link".

**For your request:**
- **Merge `d61f796c3f`,** or later if I push again; I tell you if I do.
- **Your request then carries both:** S1 lands only with the agent's call.
- **Order in the round:** your request first, so that no `main` holds `init` waiting for a call no agent makes.
