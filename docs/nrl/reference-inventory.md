# T00: reference and platform inventory

**Recorded:** 2026-10-25 · **Task:** [900-NRL-0001](../../issues-done/900-NRL-0001-reference-inventory.done) · **Source snapshot:** `dc2e4312fa4daf1f0f7c7db205627ac4c647e8e3`.

## Reference identity

- Qwen Spline Lab v0.1.0, MIT, supplied by the maintainer; the original archive stays unchanged in `nrl/reference/`.
- Archive SHA-256: `5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25`.
- 54 payload files match every entry of its internal `MANIFEST.sha256`; the manifest itself makes 55 archive entries. No model weights are included.
- `representations.py`: dense SiLU, channel pruning, piecewise-linear splines, cubic Hermite splines and SVD. The cubic prototype is not a B-spline implementation.
- `training.py`, `evaluation.py`, `artifacts.py`: training and validation selection, task evaluation, serialization. The general Rust model/format remains separate from Qwen's `hf.py`.

## Actual reproduction

[The full report](../../nrl/reports/reference-2026-10-25.json) records **64 passed, zero skipped**, Python 3.12.3, CPU PyTorch 2.10.0+cpu and Transformers 4.57.6. The unchanged suite took 1.57 seconds in this run; this is a test duration, not a model speed measurement.

The five optional Transformers checks now ran: a randomly initialized tiny Qwen3, graph replacements, standalone export/load and tied weights. No trained Qwen weights were downloaded or evaluated. HF offline flags were set; the downloader test uses its original stub. The package installation used network access before the run.

The archive's original **59 passed, 1 skipped** report of 2026-10-22 stays historical. Its skips have not been relabelled as a previous success.

## Reproduce in a fresh environment

From the repository root (choose new paths):

```sh
python3 -m venv /tmp/nrl-python
/tmp/nrl-python/bin/python -m pip install --index-url https://download.pytorch.org/whl/cpu 'torch==2.10.0+cpu'
/tmp/nrl-python/bin/python -m pip install --index-url https://pypi.org/simple -r nrl/reference/requirements-cpu.lock
/tmp/nrl-python/bin/python nrl/tools/check_reference.py --work-dir /tmp/nrl-reference --report /tmp/nrl-reference.json --tests
/tmp/nrl-python/bin/python nrl/tools/generate_fixtures.py --reference-root /tmp/nrl-reference/qwen-spline-lab --output /tmp/nrl-fixtures.json
```

The lock pins installed package versions; it is not a wheel hash lock or a promise that future package availability or other platforms are identical. The report records the environment actually run. The test runner verifies the complete archive before extracting to a new directory and never overwrites an existing directory or report.

## Differential data

`nrl/tests/fixtures/f64-v0.json` was generated twice, with identical bytes, by `generate_fixtures.py` from the verified reference source. SHA-256: `21853060ca335db5c7fc8366ad62e1b56cb89c93f507cfc4dfc1bb847f0f4fa0`.

- CPU float64, seed 42; the tensors themselves are explicitly constructed.
- Dense and spline-linear bias-free gated blocks: 3 inputs, 4 channels, 3 outputs, 3 input rows.
- Inputs, targets, outputs, MSE, input and parameter gradients, one SGD step at rate 0.05 and its new loss.
- A separate two-channel spline at internal and endpoint knots, inside intervals, and in both linear tails. This exercises input and coefficient gradients.
- The generator verifies that the imported reference files match the original archive before importing them. No trained weights or private data are used.

To check the committed fixture without changing it:

```sh
/tmp/nrl-python/bin/python nrl/tools/generate_fixtures.py --reference-root /tmp/nrl-reference/qwen-spline-lab --output nrl/tests/fixtures/f64-v0.json --check
```

## Current MIND Core integration boundary

| Area | Actual source / status | NRL consequence |
|---|---|---|
| ABI | `common/abi.rs`, ABI 20 | Rebuild against main; do not hardcode an older message's ABI |
| Named program memory | 262-KRN-0160 done; `data/memory.conf`, up to 64 GiB | An owner-authorized quota, not implicit allocation authority |
| Worker processes | 262-KRN-0163 done; `mind::process::start_worker` | Explicitly granted memory and IPC; these are not shared-address-space threads |
| Stack | 262-KRN-0164 done; 64 KiB to 8 MiB | Use the declared ELF/worker stack limit |
| Read weights | 262-KRN-0162 done; VFS 2.4 `fill` into a lent memory object | Not a general POSIX mmap or a claim of zero physical copies |
| Large pages | 262-KRN-0161 code in main; hardware measurements open | Do not transfer emulated timing to hardware |
| Existing inference | `libmind/src/nn`, `gemm`, `tests/nn_host.rs` | CPU ONNX/MINDNN01 speech inference; not an autodiff/training framework |
| Native model service | 262 APP; `nnd` and `idl/nn.wit` absent at the snapshot | Host work proceeds independently; native integration remains T11 |

Platform status above is source and issue evidence. This task ran no OS image, QEMU suite, hardware test or accelerator workload. Source-level API availability is not a native NRL result.
