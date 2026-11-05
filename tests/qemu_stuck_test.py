#!/usr/bin/env python3
"""Host test of qemu_smoke.py's record of a wait that ran out (000-KRN-0214): every CPU's registers through the monitor
and the console's end, kept as mind-core-stuck-*.log in the temporary directory, at most twice a VM (a scripted VM)."""
import importlib.util, os, pathlib, sys, tempfile, unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tests"))
spec = importlib.util.spec_from_file_location("qemu_smoke", ROOT / "tests/qemu_smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class Hung:
    """A VM whose shell echoed a line and never answered: expect() as VM's, the monitor as `answer` says."""
    def __init__(self, answer):
        self.output, self.log, self.answer, self.asked = "MIND> dmesg -s blockstore\r\n", "", answer, []
        self.process = self

    def poll(self):
        return None

    def collect(self):
        pass

    def hmp(self, command):
        self.asked.append(command)
        if isinstance(self.answer, Exception):
            raise self.answer
        return self.answer

    expect = smoke.VM.expect
    stuck = smoke.VM.stuck


class Stuck(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.before = tempfile.tempdir
        tempfile.tempdir = self.temp.name
        smoke.STUCK_AFTER = 0.05

    def tearDown(self):
        tempfile.tempdir = self.before
        self.temp.cleanup()

    def records(self):
        return sorted(pathlib.Path(self.temp.name).glob(f"mind-core-stuck-{os.getpid()}-*.log"))

    def test_a_wait_that_runs_out_keeps_the_cpus_and_the_console(self):
        vm = Hung("CPU#0\nRIP=ffffffff80001234 RFL=00000246\nCPU#1\nRIP=ffffffff80005678")
        with self.assertRaises(AssertionError):
            vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertEqual(vm.asked, ["info registers -a"])
        [record] = self.records()
        text = record.read_text()
        self.assertIn("waiting for 'MIND> '", text)
        self.assertIn("RIP=ffffffff80001234", text)
        self.assertIn("RIP=ffffffff80005678", text)
        self.assertIn("dmesg -s blockstore", text)

    def test_a_short_wait_records_nothing(self):
        vm = Hung("CPU#0")
        with self.assertRaises(AssertionError):
            vm.expect("MIND> ", timeout=0.01, after="dmesg -s blockstore\n")
        self.assertEqual((vm.asked, self.records()), ([], []))

    def test_at_most_two_records_a_vm(self):
        vm = Hung("CPU#0")
        for _ in range(4):
            with self.assertRaises(AssertionError):
                vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertEqual((len(vm.asked), len(self.records())), (2, 2))

    def test_a_monitor_that_does_not_answer_is_recorded_as_such(self):
        vm = Hung(OSError("the monitor's socket is closed"))
        with self.assertRaises(AssertionError) as raised:
            vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertIn("Timeout waiting for 'MIND> '", str(raised.exception))
        [record] = self.records()
        self.assertIn("no answer from the monitor", record.read_text())


if __name__ == "__main__":
    unittest.main()
