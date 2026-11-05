#!/usr/bin/env python3
"""Host test of qemu_smoke.py's trial_confirmed (000-KRN-0210): a trial boot's confirmation read from the console's log,
where a command whose prompt the new boot answered left it, not from the output expect() reads (a scripted console)."""
import importlib.util, pathlib, sys, unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tests"))
spec = importlib.util.spec_from_file_location("qemu_smoke", ROOT / "tests/qemu_smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)

BOOT = ("MIND> dmesg -s updater\r\nMIND CORE KERNEL: REBOOT VIA PORT 0xCF9\r\n\x1b[2JBOOT: SLOT B LOADED ON TRIAL\r\n"
        "MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED\r\nMIND> ")


class Console:
    """A VM's console: `log` and `output` as VM keeps them, and the text each collect() brings in turn."""
    def __init__(self, log, later=()):
        self.log, self.output, self.later = log, "", list(later)
        self.process = self

    def poll(self):
        return None

    def collect(self):
        if self.later:
            text = self.later.pop(0)
            self.log += text
            self.output += text


class TrialConfirmed(unittest.TestCase):
    def test_lines_a_command_took_from_the_output_are_read_from_the_log(self):
        # run 17 of 2026-11-05: a read's prompt came from the new boot, and expect() had emptied the output.
        vm = Console("before" + BOOT)
        out = smoke.trial_confirmed(vm, len("before"), 1)
        self.assertIn("MIND CORE KERNEL: REBOOT VIA PORT 0xCF9", out)
        self.assertNotIn("before", out)
        self.assertNotIn("\r", out)
        self.assertNotIn("\x1b", out)

    def test_a_confirmation_that_comes_later_is_waited_for(self):
        vm = Console("", ["MIND CORE KERNEL: REBOOT VIA PORT 0xCF9\r\n", "BOOT: SLOT B LOADED ON TRIAL\r\n", "",
                          "MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED\r\n"])
        self.assertIn("BOOT: SLOT B LOADED ON TRIAL", smoke.trial_confirmed(vm, 0, 5))
        self.assertEqual(vm.output, "")

    def test_a_confirmation_before_the_mark_since_or_before_the_trial_is_not_this_one(self):
        old = "BOOT: SLOT B LOADED ON TRIAL\r\nMIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED\r\n"
        with self.assertRaisesRegex(AssertionError, "Timeout waiting for the trial boot's confirmation"):
            smoke.trial_confirmed(Console(old + "MIND> "), len(old), .3)
        with self.assertRaisesRegex(AssertionError, "Timeout waiting for the trial boot's confirmation"):
            smoke.trial_confirmed(Console("MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED\r\nBOOT: SLOT B LOADED ON TRIAL\r\n"), 0, .3)

    def test_a_kernel_panic_ends_the_wait(self):
        with self.assertRaisesRegex(AssertionError, "KERNEL PANIC"):
            smoke.trial_confirmed(Console("", ["MIND CORE KERNEL: KERNEL PANIC: test\r\n"]), 0, 5)


if __name__ == "__main__":
    unittest.main()
