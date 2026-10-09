"""Demonstrate packaging and gate failures without building or changing the checkout."""
from pathlib import Path
import subprocess
import tempfile
import textwrap

ROOT = Path(__file__).resolve().parents[2]

with tempfile.TemporaryDirectory(prefix='mind-audit-build-') as temporary:
    base = Path(temporary)
    (base / 'common').mkdir()
    (base / 'scripts').mkdir()
    (base / 'common/abi.rs').write_bytes((ROOT / 'common/abi.rs').read_bytes())
    namespace = {'__name__': 'audit_packager', '__file__': str(base / 'scripts/make_usb_image.py')}
    source = (ROOT / 'scripts/make_usb_image.py').read_text()
    exec(compile(source, namespace['__file__'], 'exec'), namespace)
    (base / 'usb_root').mkdir()
    (base / 'usb_root/audit-app.elf').write_bytes(b'placeholder; only enumeration is exercised')
    assert 'audit-app.elf' not in namespace['files']('x86_64')
    print('PACKAGER: application created after import is omitted from files(x86_64)')

    workflow = (ROOT / '.github/workflows/ci.yml').read_text()
    start = workflow.index('      - name: Host tests\n')
    block = workflow[start:].split('        run: |\n', 1)[1].split('\n\n', 1)[0]
    block = textwrap.dedent(block)
    # All successful commands are no-ops; only the first compiler invocation fails.
    stub = '''
rustc() {
  if [[ "$*" == *tests/runtime.rs* ]]; then echo "injected rustc failure"; return 42; fi
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == -o ]]; then shift; mkdir -p "$(dirname "$1")"; printf '#!/bin/sh\\nexit 0\\n' > "$1"; chmod +x "$1"; return 0; fi
    shift
  done
}
python3() { return 0; }
'''
    # Redirect test executable paths away from any real binaries on this host.
    block = block.replace('/tmp/', str(base / 'absent-binaries') + '/')
    result = subprocess.run(['bash', '--noprofile', '--norc', '-e', '-o', 'pipefail', '-c', stub + block], cwd=ROOT, capture_output=True, text=True)
    assert 'injected rustc failure' in result.stdout, result
    assert result.returncode == 0, result
    print('GITHUB HOST STEP: rustc returned 42, but the extracted step returned 0')

    local = (ROOT / 'scripts/ci_local.sh').read_text()
    function = 'x86_fixtures() {' + local.split('x86_fixtures() {', 1)[1].split('\ntap_bench()', 1)[0]
    stub = '''
rustc() { return 0; }
cargo() { if [[ "$*" == *"--features panic-test"* ]]; then echo "injected panic-test build failure"; return 42; fi; return 0; }
'''
    result = subprocess.run(['bash', '-uo', 'pipefail', '-c', stub + function + '\nx86_fixtures'], cwd=ROOT, capture_output=True, text=True)
    assert 'injected panic-test build failure' in result.stdout, result
    assert result.returncode == 0, result
    print('LOCAL FIXTURES: panic-test cargo returned 42, but x86_fixtures returned 0')
