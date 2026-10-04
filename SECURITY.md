# Security policy

MIND Core is experimental, pre-1.0 software. It has not been audited and must not protect real data yet. What it does and does not guarantee is stated in the platform profile ([docs/profile](docs/profile/README.md), threat model in [threat-model.md](docs/profile/threat-model.md)); notably, no IOMMU is used, so any DMA-capable device can reach all memory.

## Reporting a vulnerability

Report a bug that breaks an isolation or capability guarantee of the profile (a task reaching memory, devices or capabilities it was not granted, a kernel crash from ring 3, a revocation that does not take effect) privately through GitHub: **Security → Report a vulnerability** on the repository page. Include the commit, the QEMU command line and the steps or test program that reproduce it.

You will get an acknowledgement within 14 days. Fixes are made on `main`; there are no maintained release branches yet.
