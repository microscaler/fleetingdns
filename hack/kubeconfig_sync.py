#!/usr/bin/env python3
"""
Helpers for running the FleetingDNS dev/test stack against the shared Kind
cluster on the ms02 dev host.

Two modes are supported, both NDJSON-instrumented for debug mode:

1.  REMOTE-TILT (default for `just up` on the Mac):
        The Mac opens an SSH session to ms02 with a port-forward for the
        Tilt UI (10654 by default), and runs `tilt up --context kind-kind`
        on ms02 from the sync'd repo path. Docker/kind/kind-registry stay
        on ms02; the Mac only needs SSH. No kubeconfig or apiserver tunnel
        needed.

2.  KUBECTL (optional, for running `kubectl` directly from the Mac):
        Pulls the ms02 kubeconfig to `.kube/fleetingdns.kubeconfig` and
        opens SSH `-L` forwards for the apiserver (127.0.0.1:38839) and
        kind-registry (127.0.0.1:5001). Only needed if you want to run
        kubectl/tilt against kind-kind from the Mac host.

Usage:
    python3 scripts/kubeconfig_sync.py remote-tilt-up    # (1) ssh + tilt up + 10654
    python3 scripts/kubeconfig_sync.py remote-tilt-down  # (1) ssh + tilt down
    python3 scripts/kubeconfig_sync.py remote-exec CMD   # run CMD on ms02 in repo
    python3 scripts/kubeconfig_sync.py remote-status     # kubectl get pods on ms02
    python3 scripts/kubeconfig_sync.py fetch             # (2) sync kubeconfig
    python3 scripts/kubeconfig_sync.py tunnel-up         # (2) apiserver + registry
    python3 scripts/kubeconfig_sync.py tunnel-down       # (2) close (2) tunnels
    python3 scripts/kubeconfig_sync.py status            # report both modes

Environment overrides:
    MS02_HOST            default: ms02
    MS02_SSH_USER        default: root
    MS02_REPO_PATH       default: /home/casibbald/Workspace/microscaler/fleetingdns
    KIND_CLUSTER_NAME    default: kind
    KIND_CONTEXT         default: kind-kind
    TILT_UI_PORT         default: 10654    (10348-10353 + 10450 held by other tilt stacks on ms02)
    APISERVER_PORT       default: 38839      (only used in kubectl mode)
    REGISTRY_PORT        default: 5001       (only used in kubectl mode)
    KUBECONFIG_PATH      default: <repo>/.kube/fleetingdns.kubeconfig
"""

from __future__ import annotations

import json
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parent.parent
KUBE_DIR = REPO_ROOT / ".kube"
DEFAULT_KUBECONFIG = KUBE_DIR / "fleetingdns.kubeconfig"
TUNNEL_PID_FILE = KUBE_DIR / "tunnel.pid"
TUNNEL_LOG_FILE = KUBE_DIR / "tunnel.log"
TILT_PID_FILE = KUBE_DIR / "tilt.pid"
TILT_LOG_FILE = KUBE_DIR / "tilt.log"

MS02_HOST = os.environ.get("MS02_HOST", "ms02")
MS02_SSH_USER = os.environ.get("MS02_SSH_USER", "casibbald")
MS02_REPO_PATH = os.environ.get(
    "MS02_REPO_PATH", "/home/casibbald/Workspace/microscaler/fleetingdns"
)
KIND_CLUSTER = os.environ.get("KIND_CLUSTER_NAME", "kind")
KIND_CONTEXT = os.environ.get("KIND_CONTEXT", "kind-kind")
# 10654: clear of the systemd tilt fleet on ms02 (10348-10353, 10450 taken)
TILT_UI_PORT = int(os.environ.get("TILT_UI_PORT", "10654"))
APISERVER_PORT = int(os.environ.get("APISERVER_PORT", "38839"))
REGISTRY_PORT = int(os.environ.get("REGISTRY_PORT", "5001"))
KUBECONFIG_PATH = Path(os.environ.get("KUBECONFIG_PATH", str(DEFAULT_KUBECONFIG)))

# #region agent log
DEBUG_LOG_PATH = Path(
    "/Users/casibbald/Workspace/microscaler/cylon-local-infra/.cursor/debug-c6eef8.log"
)
DEBUG_SESSION_ID = "c6eef8"


def __dbg(hypothesis: str, location: str, message: str, data: dict | None = None) -> None:
    try:
        payload = {
            "sessionId": DEBUG_SESSION_ID,
            "hypothesisId": hypothesis,
            "runId": os.environ.get("DEBUG_RUN_ID", "kubeconfig-sync"),
            "location": location,
            "message": message,
            "data": data or {},
            "timestamp": int(time.time() * 1000),
        }
        with DEBUG_LOG_PATH.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(payload) + "\n")
    except Exception:
        pass
# #endregion


def ssh_target() -> str:
    return f"{MS02_SSH_USER}@{MS02_HOST}"


def running_on_ms02() -> bool:
    try:
        host = subprocess.run(
            ["hostname", "-s"], capture_output=True, text=True, check=False
        ).stdout.strip() or subprocess.run(
            ["hostname"], capture_output=True, text=True, check=False
        ).stdout.strip()
    except Exception:
        host = ""
    return host.lower() == MS02_HOST.lower()


def run(cmd: Iterable[str], *, check: bool = True, capture: bool = True) -> subprocess.CompletedProcess[str]:
    cmd_list = list(cmd)
    print(f"🔧 {' '.join(cmd_list)}")
    proc = subprocess.run(
        cmd_list,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
    )
    if check and proc.returncode != 0:
        stdout = (proc.stdout or "").strip()
        stderr = (proc.stderr or "").strip()
        print(f"❌ command failed: {' '.join(cmd_list)}")
        if stdout:
            print(f"   stdout: {stdout}")
        if stderr:
            print(f"   stderr: {stderr}")
        sys.exit(proc.returncode)
    return proc


def ensure_kube_dir() -> None:
    KUBE_DIR.mkdir(parents=True, exist_ok=True)


# ---------------------------------------------------------------------------
# Mode 1: REMOTE-TILT — run Tilt on ms02, forward the UI to localhost
# ---------------------------------------------------------------------------

def _tilt_is_alive() -> int | None:
    if not TILT_PID_FILE.exists():
        return None
    try:
        pid = int(TILT_PID_FILE.read_text().strip())
    except ValueError:
        return None
    try:
        os.kill(pid, 0)
    except OSError:
        return None
    return pid


def remote_tilt_up() -> None:
    ensure_kube_dir()
    existing = _tilt_is_alive()
    if existing:
        print(f"ℹ️  remote tilt already running (ssh pid {existing}); visit http://localhost:{TILT_UI_PORT}")
        return
    target = ssh_target()
    # Bind 0.0.0.0 so the Tilt UI is reachable on the LAN (e.g. http://ms02:10654/).
    # Mac SSH -L still works: localhost:10654 → ms02 loopback → same listener.
    remote_cmd = (
        f"set -e; cd {MS02_REPO_PATH}; "
        f"echo '>>> just up on ms02 (shared-k8s default)'; "
        f"just up; "
        f"echo 'Tilt UI tunnel active (systemd). Ctrl+C closes SSH only.'; "
        f"exec sleep infinity"
    )
    ssh_cmd = [
        "ssh",
        "-T",
        "-o", "ExitOnForwardFailure=yes",
        "-o", "ServerAliveInterval=30",
        "-o", "ServerAliveCountMax=3",
        "-L", f"{TILT_UI_PORT}:127.0.0.1:{TILT_UI_PORT}",
        target,
        remote_cmd,
    ]
    print(f"🚀 Starting Tilt on {target} (repo={MS02_REPO_PATH}, context={KIND_CONTEXT})")
    print(f"   Tilt UI will be available at http://localhost:{TILT_UI_PORT}")
    with TILT_LOG_FILE.open("ab") as log:
        proc = subprocess.Popen(
            ssh_cmd,
            stdout=log,
            stderr=log,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
        )
    # Wait briefly to confirm the tunnel didn't exit immediately (e.g. auth
    # failure, no route to host, tilt failed to start).
    for _ in range(30):
        time.sleep(0.25)
        if proc.poll() is not None:
            break
        # Look for the Tilt listening banner in the log; bail early once up.
        if TILT_LOG_FILE.exists():
            tail = _tail_bytes(TILT_LOG_FILE, 4096).decode("utf-8", errors="replace")
            if "Tilt started" in tail or "Starting Tilt" in tail or "Api server listening" in tail:
                break
    if proc.poll() is not None:
        print(f"❌ ssh/tilt exited immediately (code {proc.returncode}); see {TILT_LOG_FILE}")
        __dbg(
            "M-remote",
            "scripts/kubeconfig_sync.py:remote_tilt_up",
            "remote tilt ssh session exited immediately",
            {"rc": proc.returncode, "log": str(TILT_LOG_FILE)},
        )
        sys.exit(1)
    TILT_PID_FILE.write_text(str(proc.pid))
    __dbg(
        "M-remote",
        "scripts/kubeconfig_sync.py:remote_tilt_up",
        "remote tilt session started; UI forwarded",
        {
            "ssh_pid": proc.pid,
            "host": MS02_HOST,
            "repo": MS02_REPO_PATH,
            "context": KIND_CONTEXT,
            "tilt_ui_port": TILT_UI_PORT,
        },
    )
    print(f"✅ ssh+tilt pid {proc.pid} — Tilt UI: http://localhost:{TILT_UI_PORT}")
    print(f"   Logs: {TILT_LOG_FILE}")


def remote_tilt_down() -> None:
    # First, try to stop the remote tilt process cleanly so it has a chance
    # to clean up its k8s resources.
    target = ssh_target()
    print(f"🛑 Asking Tilt on {target} to shut down")
    subprocess.run(
        [
            "ssh", "-o", "ConnectTimeout=10", target,
            f"cd {MS02_REPO_PATH} && systemctl --user stop tilt-fleetingdns.service 2>/dev/null; "
            f"tilt down --port {TILT_UI_PORT} 2>/dev/null || true",
        ],
        check=False,
    )
    # Then, tear down the SSH session that runs `tilt up`.
    pid = _tilt_is_alive()
    if pid is None:
        if TILT_PID_FILE.exists():
            TILT_PID_FILE.unlink()
        print("ℹ️  No local ssh+tilt session tracked.")
        __dbg(
            "M-remote",
            "scripts/kubeconfig_sync.py:remote_tilt_down",
            "no local ssh+tilt session tracked; remote tilt down invoked only",
            {},
        )
        return
    try:
        os.killpg(os.getpgid(pid), signal.SIGTERM)
    except OSError:
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    for _ in range(20):
        time.sleep(0.1)
        try:
            os.kill(pid, 0)
        except OSError:
            break
    TILT_PID_FILE.unlink(missing_ok=True)
    __dbg(
        "M-remote",
        "scripts/kubeconfig_sync.py:remote_tilt_down",
        "ssh+tilt session stopped",
        {"pid": pid},
    )
    print(f"✅ ssh+tilt pid {pid} stopped.")


def remote_exec(argv: list[str]) -> None:
    if not argv:
        print("usage: kubeconfig_sync.py remote-exec <command...>")
        sys.exit(2)
    target = ssh_target()
    remote_cmd = f"cd {MS02_REPO_PATH} && " + " ".join(argv)
    subprocess.run(["ssh", target, remote_cmd], check=False)


def remote_status() -> None:
    target = ssh_target()
    print(f"📊 Remote status on {target} ({MS02_REPO_PATH}):")
    subprocess.run(
        [
            "ssh", "-o", "ConnectTimeout=5", target,
            f"cd {MS02_REPO_PATH} && "
            f"echo '== kind clusters ==' && kind get clusters && "
            f"echo '== current-context ==' && kubectl config current-context && "
            f"echo '== nodes ==' && kubectl --context {KIND_CONTEXT} get nodes && "
            f"echo '== fleetingdns pods ==' && (kubectl --context {KIND_CONTEXT} get pods -n fleetingdns 2>&1 || echo '(namespace not yet created)') && "
            f"echo '== tilt get resources ==' && (tilt get resources 2>&1 | head -30 || echo '(tilt not running on host)')",
        ],
        check=False,
    )


# ---------------------------------------------------------------------------
# Mode 2: KUBECTL tunnel — optional utility to use kubectl from the Mac
# ---------------------------------------------------------------------------

def fetch_kubeconfig() -> Path:
    ensure_kube_dir()
    target = ssh_target()
    print(f"📥 Fetching kubeconfig for kind cluster '{KIND_CLUSTER}' from {target}")
    proc = run(
        [
            "ssh", "-o", "ConnectTimeout=5", target,
            f"kind get kubeconfig --name {KIND_CLUSTER}",
        ]
    )
    kubeconfig = proc.stdout
    if "apiVersion: v1" not in kubeconfig:
        print("❌ Received kubeconfig does not look valid.")
        print(kubeconfig)
        sys.exit(1)
    KUBECONFIG_PATH.write_text(kubeconfig, encoding="utf-8")
    os.chmod(KUBECONFIG_PATH, 0o600)
    __dbg(
        "M-kubectl",
        "scripts/kubeconfig_sync.py:fetch_kubeconfig",
        "wrote ms02 kubeconfig to repo-scoped path",
        {"path": str(KUBECONFIG_PATH), "bytes": len(kubeconfig)},
    )
    print(f"✅ Wrote {KUBECONFIG_PATH} (mode 0600)")
    return KUBECONFIG_PATH


def _kubectl_tunnel_is_alive() -> int | None:
    if not TUNNEL_PID_FILE.exists():
        return None
    try:
        pid = int(TUNNEL_PID_FILE.read_text().strip())
    except ValueError:
        return None
    try:
        os.kill(pid, 0)
    except OSError:
        return None
    return pid


def kubectl_tunnel_up() -> None:
    ensure_kube_dir()
    existing = _kubectl_tunnel_is_alive()
    if existing:
        print(f"ℹ️  kubectl tunnel already running (pid {existing})")
        return
    target = ssh_target()
    ssh_cmd = [
        "ssh",
        "-N", "-T",
        "-o", "ExitOnForwardFailure=yes",
        "-o", "ServerAliveInterval=30",
        "-o", "ServerAliveCountMax=3",
        "-L", f"{APISERVER_PORT}:127.0.0.1:{APISERVER_PORT}",
        "-L", f"{REGISTRY_PORT}:127.0.0.1:{REGISTRY_PORT}",
        target,
    ]
    print(f"🚇 Opening kubectl-mode SSH tunnel: {' '.join(ssh_cmd)}")
    with TUNNEL_LOG_FILE.open("ab") as log:
        proc = subprocess.Popen(
            ssh_cmd,
            stdout=log,
            stderr=log,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
        )
    time.sleep(1.2)
    if proc.poll() is not None:
        print(f"❌ kubectl tunnel exited immediately (code {proc.returncode}); see {TUNNEL_LOG_FILE}")
        sys.exit(1)
    TUNNEL_PID_FILE.write_text(str(proc.pid))
    __dbg(
        "M-kubectl",
        "scripts/kubeconfig_sync.py:kubectl_tunnel_up",
        "apiserver+registry tunnel started",
        {
            "pid": proc.pid,
            "apiserver_port": APISERVER_PORT,
            "registry_port": REGISTRY_PORT,
        },
    )
    print(f"✅ kubectl tunnel pid {proc.pid} (apiserver :{APISERVER_PORT}, registry :{REGISTRY_PORT})")


def kubectl_tunnel_down() -> None:
    pid = _kubectl_tunnel_is_alive()
    if pid is None:
        if TUNNEL_PID_FILE.exists():
            TUNNEL_PID_FILE.unlink()
        print("ℹ️  No active kubectl tunnel.")
        return
    try:
        os.killpg(os.getpgid(pid), signal.SIGTERM)
    except OSError:
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    for _ in range(20):
        time.sleep(0.1)
        try:
            os.kill(pid, 0)
        except OSError:
            break
    TUNNEL_PID_FILE.unlink(missing_ok=True)
    print(f"✅ kubectl tunnel pid {pid} stopped.")


# ---------------------------------------------------------------------------
# Status / helpers
# ---------------------------------------------------------------------------

def _tail_bytes(path: Path, n: int) -> bytes:
    try:
        size = path.stat().st_size
    except OSError:
        return b""
    with path.open("rb") as fh:
        if size > n:
            fh.seek(size - n)
        return fh.read()


def status() -> None:
    print("📊 FleetingDNS shared-cluster status")
    tilt_pid = _tilt_is_alive()
    if tilt_pid:
        print(f"  remote Tilt:        ✅ ssh pid {tilt_pid} — http://localhost:{TILT_UI_PORT}")
    else:
        print("  remote Tilt:        ❌ not running   (`just up` or `python3 scripts/kubeconfig_sync.py remote-tilt-up`)")
    tun_pid = _kubectl_tunnel_is_alive()
    if tun_pid:
        print(f"  kubectl tunnel:     ✅ pid {tun_pid}")
    else:
        print("  kubectl tunnel:     (optional — use `just kubectl-tunnel-up`)")
    if KUBECONFIG_PATH.exists():
        print(f"  kubeconfig:         ✅ {KUBECONFIG_PATH}")
    else:
        print(f"  kubeconfig:         (optional — use `just kubeconfig-sync`)")


COMMANDS = {
    "remote-tilt-up": lambda argv: remote_tilt_up(),
    "remote-tilt-down": lambda argv: remote_tilt_down(),
    "remote-exec": lambda argv: remote_exec(argv),
    "remote-status": lambda argv: remote_status(),
    "fetch": lambda argv: fetch_kubeconfig(),
    "tunnel-up": lambda argv: kubectl_tunnel_up(),
    "tunnel-down": lambda argv: kubectl_tunnel_down(),
    "status": lambda argv: status(),
}


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        print(__doc__)
        print("available commands: " + ", ".join(sorted(COMMANDS)))
        sys.exit(2)
    COMMANDS[sys.argv[1]](sys.argv[2:])


if __name__ == "__main__":
    main()
