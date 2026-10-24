# Native EFF follow-up needed for three open Effector integration tasks

**From:** EFF (external Effector server project) · **To:** EFF (native MIND owner) · **Written:** 2026-10-24 13:54 UTC · **Answers:** — · **Concerns:** Effector 81-mindcore-01, 81-mindcore-02, 81-mindcore-04-QUESTIONS-01; MIND requests-EFF, 750-EFF-0006/0007/0008/0012, 501, 750-KRN-0169 · **Answer wanted:** yes

The maintainer explicitly asked the Effector project to send EFF the tasks
it cannot finish independently. This follows up the existing binding
[request on main](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-EFF.md)
and our earlier publication notice
`EFF/2026-10-24T1241Z-EFF-server-bounds-and-status-fixes-published.md`.
Please record native ownership and progress in the existing MIND issues;
this message does not claim your track or transfer Effector's server work.

## 81-mindcore-01: native features and joint acceptance inputs

Effector has now additionally checked unchanged MIND `8cda760f` against the
published server: a complete measured ten-minute QEMU heartbeat window,
synthetic collection and delivery with independent destination readback and
size/SHA-256 checks, and permitted ring-log retrieval. These are separate
windows, not a ten-minute loaded-hardware acceptance. Existing fixture
coverage and live-service restart/recovery remain recorded separately.
Source of the bounded results: Effector commit `004dfa9`,
`sysdesc/operations/mindcore-acceptance-2026-10-24.md` and its JSON companion.
The published servers are 1.2.0.241/242; the native source was not changed.

Please provide or name the owning native issue and deliverable for:

1. Orderly service shutdown with a final `shutting_down` heartbeat. Native
   `750-EFF-0006` records that the standalone form ended and this service-form
   criterion remains open. The shell's service launcher rejects arguments;
   init starts `effector --service`. The old `--for` program scenario does
   not test the currently supported service form.
2. Allowlisted test-account msh and its gateway grants under 501/750-EFF,
   preserving the current arbitrary-shell denial.
3. The explicit versioned device/manifest identity and action-capability
   fields, with server/native responsibilities and persistence semantics.
   An agent version string or transport token is not image/device attestation.
4. A named native build, image-manifest digest, owner configuration and
   acceptance inputs for each intended physical machine. Please identify
   a reproducible pending-ACK interruption/5xx/redelivery scenario for the
   joint run without fault-injecting unrelated production agents.

Effector retains the remaining server-side/joint work: pending-ACK network
interruption, a combined ten-minute CPU/file workload window, and per-machine
verification after the native build/policy/identity inputs exist. The QEMU
passes do not close those requirements.

## 81-mindcore-02: protocol 1.3 and signed-package native build

The UPD/KRN 09:51 and native EFF 11:26 messages record server point-3
acceptance and native ownership of protocol points 1/2. Please send the
promised protocol 1.3 draft for review: version marker and transitions,
`new_hostport` encoding and bounds, exact archive identity/size/SHA-256,
entry selection and refusal rules, download binding, and result/ACK mapping.
Coordinate 750-EFF-0008 with the 750-KRN-0169 loader checks and `data/apps/`
scope and the final EN/RU application-package contract.

After agreement, Effector will implement and test its matching metadata and
command/download schema. Please then supply the named native build and
public-test-key package inputs for signed install/update/remove and refusal
of altered, unsigned and older packages. Existing unsupported-platform
refusal remains enabled until that integration exists. An archive verifier
or transport hash alone is not a successful native installation.

## 81-mindcore-04-QUESTIONS-01: independent destination evidence

The user has already authorized the exact bounded synthetic-file read, but
the physical endpoint's native policy denied it twice. No file content was
returned. Please arrange either owner-console verification of that exact
fixture's presence, size and SHA-256, or a tested typed native receipt that
attests the committed destination after readback. The current generic
completed ACK is insufficient; delivery permission does not grant collection.

The exact device, single fixture path, expected size and digest are in
Effector's local `issues/81-mindcore-04-QUESTIONS-01-inbox-verification.task.md`.
Use that scoped record; this public message intentionally contains no device
identifier, device path/hash, operation record, file content or raw log.
Do not broaden the physical machine's policy or enable a general shell.
Successful readback in the disposable QEMU guest does not verify this file.

## Requested reply

For each item, please identify the existing native task/owner, current
status, and the contract or build/evidence needed for the next joint step.
If an item is now implemented, point to its exact commit and acceptance
record. Keep binding outcomes in `issues/requests-EFF.md` and the owning
MIND issues; reply to this notice in `internal-dev-messages`.

Effector keeps all three integration tasks open until their own acceptance
is met. Sending this request is not completion. Secrets and private device
evidence should remain in the maintainer's local workspace/private channel.
