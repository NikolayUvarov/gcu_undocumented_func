#!/usr/bin/env python3
"""Host test of qemu_smoke.py's record of a wait that ran out (000-KRN-0214): every CPU's registers through the monitor,
each task's state from the kernel's page of them (000-KRN-0232) and the console's end, kept as mind-core-stuck-*.log in
the temporary directory, at most twice a VM (a scripted VM)."""
import importlib.util, os, pathlib, sys, tempfile, unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tests"))
spec = importlib.util.spec_from_file_location("qemu_smoke", ROOT / "tests/qemu_smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class Hung:
    """A VM whose shell echoed a line and never answered: expect() as VM's, the monitor as `answer` says; `memory`
    holds what memsave and pmemsave copy, by command."""
    def __init__(self, answer, log="", memory=None):
        self.output, self.log, self.answer, self.asked = "MIND> dmesg -s blockstore\r\n", log, answer, []
        self.memory = memory or {}
        self.process = self

    def poll(self):
        return None

    def collect(self):
        pass

    def hmp(self, command):
        self.asked.append(command)
        if isinstance(self.answer, Exception):
            raise self.answer
        if command.split()[0] in ("memsave", "pmemsave"):
            pathlib.Path(command.split('"')[1]).write_bytes(self.memory.get(command.split()[0], b""))
            return ""
        return self.answer

    expect = smoke.VM.expect
    stuck = smoke.VM.stuck
    task_states = smoke.VM.task_states


BOOT = "MIND CORE KERNEL: INIT STARTED\r\nMIND CORE KERNEL: TASK STATES AT 0x3A5000, 16384 BYTES, EACH SECOND\r\n"
STATES = b"AT 41000 MS\n1 init CPU 0 IPC_WAIT RECEIVE ON 1\n52 shell CPU 2 IPC_WAIT REPLY FROM 30 loader\n30 loader CPU 1 IPC_WAIT SEND TO 9 OF 18 vfs_server\n\0garbage"


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

    def test_the_record_holds_each_tasks_state_from_the_kernels_page(self):
        vm = Hung("CPU#0", log=BOOT, memory={"memsave": STATES})
        with self.assertRaises(AssertionError):
            vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertEqual(vm.asked[0], "info registers -a")
        self.assertRegex(vm.asked[1], r'^memsave 0x3a5000 16384 ".*mind-core-states-\d+\.bin"$')
        self.assertEqual(len(vm.asked), 2)
        [record] = self.records()
        text = record.read_text()
        self.assertIn("--- the tasks\nAT 41000 MS\n", text)
        self.assertIn("52 shell CPU 2 IPC_WAIT REPLY FROM 30 loader", text)
        self.assertIn("30 loader CPU 1 IPC_WAIT SEND TO 9 OF 18 vfs_server", text)
        self.assertNotIn("garbage", text)
        self.assertEqual(list(pathlib.Path(self.temp.name).glob("mind-core-states-*")), [])

    def test_a_page_not_mapped_is_read_by_its_physical_address(self):
        vm = Hung("CPU#0", log=BOOT, memory={"memsave": b"\xff\xff", "pmemsave": STATES})
        with self.assertRaises(AssertionError):
            vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertEqual([command.split()[0] for command in vm.asked], ["info", "memsave", "pmemsave"])
        [record] = self.records()
        self.assertIn("52 shell CPU 2 IPC_WAIT REPLY FROM 30 loader", record.read_text())

    def test_a_kernel_that_printed_no_address_is_recorded_as_such(self):
        vm = Hung("CPU#0")
        with self.assertRaises(AssertionError):
            vm.expect("MIND> ", timeout=0.1, after="dmesg -s blockstore\n")
        self.assertEqual(vm.asked, ["info registers -a"])
        [record] = self.records()
        self.assertIn("--- the tasks\nthe kernel printed no address for them\n", record.read_text())

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
