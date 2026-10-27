import contextlib
import io
import json
from pathlib import Path
import runpy
import types
import unittest
from unittest.mock import patch


PROBE = Path(__file__).resolve().parents[1] / "assets" / "monitor" / "linux.py"


class ProbeTests(unittest.TestCase):
    def setUp(self):
        self.cpu_reads = 0
        self.process_reads = 0
        self.missing_memory = False
        self.vanished_process = False

    def open(self, path, **kwargs):
        if path == "/proc/stat":
            self.cpu_reads += 1
            factor = self.cpu_reads
            return io.StringIO(f"cpu {10 * factor} 0 {10 * factor} {80 * factor} 0 0 0 0\n")
        if path == "/proc/1/stat":
            if self.vanished_process:
                raise FileNotFoundError(path)
            self.process_reads += 1
            fields = ["0"] * 22
            fields[0] = "S"
            fields[11] = str(10 * self.process_reads)
            fields[19] = "123"
            fields[21] = "2"
            return io.StringIO("1 (a process) " + " ".join(fields))
        if path == "/proc/meminfo":
            available = "" if self.missing_memory else "MemAvailable: 4096 kB\n"
            return io.StringIO(
                "MemTotal: 8192 kB\n" + available + "SwapTotal: 1024 kB\nSwapFree: 512 kB\n"
            )
        files = {
            "/proc/uptime": "80000.50 500.0",
            "/proc/net/dev": "header\nheader\n eth0: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n",
            "/proc/self/mounts": (
                "/dev/vda1 / ext4 rw 0 0\n"
                "tmpfs /mnt/a\\040b tmpfs rw 0 0\n"
                "proc /proc proc rw 0 0\n"
            ),
        }
        return io.StringIO(files[path])

    def run_probe(self, system="Linux"):
        output, errors = io.StringIO(), io.StringIO()
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch("builtins.open", self.open))
            stack.enter_context(patch("platform.system", return_value=system))
            stack.enter_context(patch("platform.release", return_value="6.12"))
            stack.enter_context(patch("socket.gethostname", return_value="fixture"))
            stack.enter_context(patch("os.listdir", return_value=["1", "self", "net"]))
            stack.enter_context(patch("os.sysconf", side_effect=lambda key: 100 if key == "SC_CLK_TCK" else 4096))
            stack.enter_context(patch("os.getloadavg", return_value=(0.03, 0.09, 0.08)))
            stack.enter_context(patch("os.statvfs", return_value=types.SimpleNamespace(
                f_blocks=100, f_frsize=4096, f_bavail=50,
            )))
            stack.enter_context(patch("time.sleep"))
            stack.enter_context(patch("time.monotonic", side_effect=[10.0, 10.5]))
            stack.enter_context(contextlib.redirect_stdout(output))
            stack.enter_context(contextlib.redirect_stderr(errors))
            status = 0
            try:
                runpy.run_path(str(PROBE), run_name="__main__")
            except SystemExit as error:
                status = error.code
        return status, output.getvalue(), errors.getvalue()

    def test_linux_counters_are_sampled_as_deltas(self):
        status, output, errors = self.run_probe()
        self.assertEqual(status, 0, errors)
        data = json.loads(output)
        self.assertAlmostEqual(data["cpu"], 20)
        self.assertAlmostEqual(data["processes"][0]["cpu"], 20)
        self.assertEqual(data["processes"][0]["memory"], 8192)
        self.assertEqual(data["memory"]["used"], 4096 * 1024)
        self.assertEqual(data["networks"][0]["transmitted"], 200)

    def test_mount_escapes_are_decoded_and_virtual_filesystems_excluded(self):
        status, output, errors = self.run_probe()
        self.assertEqual(status, 0, errors)
        self.assertEqual([disk["path"] for disk in json.loads(output)["disks"]], ["/", "/mnt/a b"])

    def test_disappearing_process_does_not_fail_the_sample(self):
        self.vanished_process = True
        status, output, errors = self.run_probe()
        self.assertEqual(status, 0, errors)
        self.assertEqual(json.loads(output)["processes"], [])

    def test_missing_required_memory_information_fails_explicitly(self):
        self.missing_memory = True
        status, output, errors = self.run_probe()
        self.assertEqual(status, 1)
        self.assertEqual(output, "")
        self.assertIn("MemAvailable", errors)

    def test_unsupported_remote_os_is_not_reported_as_linux(self):
        status, output, errors = self.run_probe("Darwin")
        self.assertEqual(status, 1)
        self.assertEqual(output, "")
        self.assertIn("requires a Linux server", errors)


if __name__ == "__main__":
    unittest.main()
