import json
import os
import platform
import re
import socket
import sys
import time


def read(path):
    with open(path, encoding="utf-8", errors="replace") as source:
        return source.read()


def cpu():
    values = [int(value) for value in read("/proc/stat").splitlines()[0].split()[1:9]]
    return sum(values), values[3] + values[4]


def processes():
    result = {}
    for name in os.listdir("/proc"):
        if not name.isdigit():
            continue
        try:
            stat = read("/proc/" + name + "/stat")
            command, fields = stat[stat.index("(") + 1:].rsplit(")", 1)
            fields = fields.split()
            result[int(name)] = {
                "command": command[:4096],
                "ticks": int(fields[11]) + int(fields[12]),
                "start": int(fields[19]),
                "memory": max(0, int(fields[21])) * os.sysconf("SC_PAGE_SIZE"),
            }
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    return result


def disks():
    kinds = {
        "ext2", "ext3", "ext4", "xfs", "btrfs", "zfs", "overlay", "tmpfs",
        "devtmpfs", "vfat", "exfat", "f2fs", "squashfs", "rootfs",
    }
    result = []
    seen = set()
    for line in read("/proc/self/mounts").splitlines():
        fields = line.split()
        path = re.sub(r"\\([0-7]{3})", lambda match: chr(int(match[1], 8)), fields[1])
        if fields[2] not in kinds or path in seen:
            continue
        seen.add(path)
        try:
            stat = os.statvfs(path)
        except (FileNotFoundError, PermissionError):
            continue
        total = stat.f_blocks * stat.f_frsize
        available = max(0, stat.f_bavail) * stat.f_frsize
        result.append({
            "path": path[:4096],
            "usage": {"total": total, "used": max(0, total - available)},
        })
    return sorted(result, key=lambda disk: disk["path"])[:256]


def sample():
    if platform.system() != "Linux":
        raise RuntimeError("remote monitoring currently requires a Linux server")
    before_cpu = cpu()
    before_processes = processes()
    then = time.monotonic()
    time.sleep(0.3)
    after_cpu = cpu()
    elapsed = time.monotonic() - then
    ticks = os.sysconf("SC_CLK_TCK")
    running = []
    for pid, process in processes().items():
        previous = before_processes.get(pid)
        usage = 0.0
        if previous and previous["start"] == process["start"]:
            usage = max(0, process["ticks"] - previous["ticks"]) / ticks / elapsed * 100
        running.append({
            "pid": pid, "command": process["command"],
            "memory": process["memory"], "cpu": usage,
        })
    by_cpu = sorted(running, key=lambda process: (-process["cpu"], -process["memory"]))[:128]
    by_memory = sorted(running, key=lambda process: -process["memory"])[:128]
    running = list({process["pid"]: process for process in by_cpu + by_memory}.values())
    memory = {}
    for line in read("/proc/meminfo").splitlines():
        name, value = line.split(":", 1)
        memory[name] = int(value.split()[0]) * 1024
    total = memory["MemTotal"]
    available = memory["MemAvailable"]
    swap = memory["SwapTotal"]
    networks = []
    for line in read("/proc/net/dev").splitlines()[2:]:
        name, values = line.split(":", 1)
        name = name.strip()
        if name == "lo":
            continue
        values = values.split()
        networks.append({
            "name": name, "received": int(values[0]), "transmitted": int(values[8]),
        })
    total_ticks = after_cpu[0] - before_cpu[0]
    idle_ticks = after_cpu[1] - before_cpu[1]
    if total_ticks <= 0:
        raise RuntimeError("CPU counters did not advance")
    return {
        "hostname": socket.gethostname(),
        "os": platform.system() + " " + platform.release(),
        "uptime": int(float(read("/proc/uptime").split()[0])),
        "load": list(os.getloadavg()),
        "cpu": min(100, max(0, (1 - idle_ticks / total_ticks) * 100)),
        "memory": {"used": max(0, total - available), "total": total},
        "swap": {"used": swap - memory["SwapFree"], "total": swap},
        "processes": running,
        "networks": networks[:256],
        "disks": disks(),
    }


try:
    print(json.dumps(sample(), allow_nan=False))
except Exception as error:
    print("monitor: Linux collector failed: " + str(error), file=sys.stderr)
    sys.exit(1)
