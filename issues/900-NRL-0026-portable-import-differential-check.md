# 900-NRL-0026: Differential checks of live NRL exports through APP's portable importer

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress (2026-10-30) · **Blocked by:** — for portable host verification; native acceptance remains 900-NRL-0017 / 262-APP-0112 · **Roadmap:** G; 900 T11 · **Constitution:** MC-5.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

APP accepted NRL's format/lifetime clarifications, implemented nnpkg/nnd/nn.wit, and owns its native parser and client integration. KRN's boot grants landed in round 50. The fixed 0025 fixture is covered by APP's host tests; fresh exports with other shapes, optional biases, transformed blocks, lineage and boundary sizes need direct differential checking against the actual portable importer. A service boot/grant test alone does not close native T11.

## Plan

1. Add an NRL host command that reads a current standalone export, checks it through APP's direct and isolated-parser-description APIs, reconstructs the job-worker description and compares graph layout, outputs and resource charges with NRL's host model/runtime.
2. Execute the portable service's submit/start/finish/take path and cleanup on the host. State that this is in-process portable-code evidence, without native IPC, badge authentication, worker isolation or VFS-scope claims.
3. Exercise genuinely different live exports: all five representations, dimensions, optional biases, spline knots, SVD ranks, pruning, graph outputs, materialized history, metadata and declared object/node/value boundaries. Preserve the writer-subset refusal and IPC payload limits.
4. Run affected checks and portable builds; update current NRL dependency/status records and notify APP/KRN with concrete verified results and remaining native work. Change no APP/KRN implementation.

## Acceptance criteria

- [ ] A bounded offline CLI checks live exports against APP's actual portable importer/parser layouts/service execution and writes an immutable report.
- [ ] Differential cases cover the specified exporter variations/bounds with matching identities/layout/results/charges and named unsupported-input refusals.
- [ ] Affected NRL and nnpkg checks and portable builds pass; evidence and status record exact commits/configurations and preserve open native 0017 acceptance.
- [ ] APP/KRN receive the results and the remaining native integration request; completed task and index are consistent.

## Related

- [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md), [900](900-neural-representation-lab.md).
- [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md).
- [0025](../issues-done/900-NRL-0025-native-conformance-and-app-handoff.done), [native handoff](../nrl/specs/native-conformance-v0.md).
