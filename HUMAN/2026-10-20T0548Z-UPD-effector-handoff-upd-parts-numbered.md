# Effector handoff: the update track's parts are numbered, the contract's draft 0.1 is written

**From:** UPD · **To:** HUMAN · **Written:** 2026-10-20 05:48 UTC · **Answers:** ALL/2026-10-15T2016Z-HUMAN-effector-native-agent-and-package-handoff.md · **Concerns:** 750, requests-EFF.md, 750-UPD-0019, 750-UPD-0020; `main` 1753bf46 · **Answer wanted:** yes (who takes `EFF`)

## Who handles what

- **The update track (`UPD`)**, worked by this session, takes the signed-application contract and the updater's side. The two requests from `EFF` in `requests-UPD.md` are now tasks:
  - [750-UPD-0019](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-UPD-0019-signed-application-packages.md): signed application packages: the contract, the key role "application", one check shared by the gateway and the loader, a signing tool;
  - [750-UPD-0020](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-UPD-0020-the-gateways-client-of-the-updater.md): the gateway's client of `updater` (check, fetch, status; apply and rollback only with `init`'s arm).
- **`EFF` itself has no owner.** The agent and the gateway (0003, 0006, 0007, 0008, 0012) wait for one. Nobody has started them: the branch `claude/EFF-effector-agent` holds only the issues and docs already on `main`. Whether this session takes `EFF` beside `NET`, `STO` and `UPD` is your call.
- **`KRN`'s parts** stay its own: `init`'s grants, the scoped `vfs` client, the gateway's badge for the updater, the loader's check of an installed application.

## The contract: location and version

- **Draft 0.1** is the Plan of 750-UPD-0019 on `main` (1753bf46). It is a proposal to `EFF`, `KRN` and the Effector server, not an agreement. What it says:
  - one package per application and architecture, under `mindcore/<arch>/` in the deployment ZIP;
  - the canonical signed bytes are `MANIFEST`'s exact bytes, a fixed line format with stated bounds;
  - an Ed25519 signature over a prefix of its own and those bytes, so that it cannot pass as a boot manifest's or a channel's;
  - application keys trusted per name in the owner's policy;
  - install, update and uninstall as actions, with no scripts;
  - no downgrade without the policy's word;
  - the loader's check before a program from `data/apps/` runs.
- **Binding the exact archive** (point 3 of the request): the command's size and SHA-256 of the ZIP are checked before it is opened. The signature, not that hash, decides whether its contents may be installed.
- **Version 1.0** will be `docs/update/application-packages.md`, in English and Russian, once agreed. A later change is a new format version with a stated transition.

## The first build for joint acceptance

There is none. Nothing of the native agent exists yet. On the update side, the first build will hold the shared check and the signing tool with host tests. It proves the format, not an install: no gateway exists to install with. A build the Effector server can test against needs `EFF`'s agent and gateway first.

## What is wanted from you

- Who takes `EFF`.
- Comments on draft 0.1, from you or the Effector server project, in reply to this message or on 750-UPD-0019.
