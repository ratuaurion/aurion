import subprocess
import json
import base64

def check():
    print("=" * 70)
    print("  FORENSIK VERIFIKASI BINARY AURION (RUNNING vs WORKSPACE)")
    print("=" * 70)

    # 1. VPS Binary & Running Processes
    vps_inspect_sh = """
echo "=== VPS FILE ==="
ls -l --time-style=full-iso /usr/local/bin/aurion
sha256sum /usr/local/bin/aurion
/usr/local/bin/aurion version

echo "=== VPS RUNNING PIDS ==="
for p in $(pgrep -f "aurion"); do
    exe=$(readlink -f /proc/$p/exe 2>/dev/null)
    cmd=$(tr '\\0' ' ' < /proc/$p/cmdline 2>/dev/null)
    if [[ "$cmd" != *"python"* && -n "$exe" ]]; then
        sh=$(sha256sum $exe | awk '{print $1}')
        echo "PID $p: $exe | sha256: $sh"
        echo "   cmd: $cmd"
    fi
done
"""
    b64 = base64.b64encode(vps_inspect_sh.encode()).decode()
    cmd = ["wsl.exe", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", f"echo {b64} | base64 -d | bash"]
    res_vps = subprocess.run(cmd, text=True, capture_output=True)
    print(res_vps.stdout)

    # 2. Docker Containers
    print("=== DOCKER CONTAINERS (LAPTOP) ===")
    cmd_docker = ["wsl.exe", "docker", "run", "--rm", "--entrypoint", "sha256sum", "aurion:latest", "/bin/aurion"]
    sh_docker = subprocess.run(cmd_docker, text=True, capture_output=True).stdout.strip()
    print("Image 'aurion:latest' /bin/aurion sha256:", sh_docker)

    cmd_docker_ver = ["wsl.exe", "docker", "run", "--rm", "aurion:latest", "version"]
    ver_docker = subprocess.run(cmd_docker_ver, text=True, capture_output=True).stdout.strip()
    print("Image 'aurion:latest' version output:\n", ver_docker)

    # 3. WSL Local Binary
    print("\n=== WSL HOST LOCAL (/usr/local/bin/aurion) ===")
    cmd_wsl = ["wsl.exe", "bash", "-c", "ls -l --time-style=full-iso /usr/local/bin/aurion; sha256sum /usr/local/bin/aurion"]
    print(subprocess.run(cmd_wsl, text=True, capture_output=True).stdout.strip())

    # 4. Target release binaries
    print("\n=== LOCAL TARGET RELEASE BINARY ===")
    cmd_target = ["wsl.exe", "bash", "-c", "ls -l --time-style=full-iso /mnt/c/Projects/aurion/target/release/aurion* 2>/dev/null; sha256sum /mnt/c/Projects/aurion/target/release/aurion 2>/dev/null"]
    print(subprocess.run(cmd_target, text=True, capture_output=True).stdout.strip())

if __name__ == "__main__":
    check()
