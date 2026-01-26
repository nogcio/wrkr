# Devcontainer notes

## `perf` profiling (Linux)

The profiling helper (`wrkr-tools-profile`) uses Linux `perf` inside this devcontainer.

For `perf record` to work reliably in a container, you generally need both:

1) Container runtime permissions (already configured in `.devcontainer/devcontainer.json`):
   - `--cap-add=SYS_ADMIN`
   - `--cap-add=SYS_PTRACE`
   - `--security-opt=seccomp=unconfined`

2) Host kernel settings (must be set on the Docker host kernel, not inside the container).

   Note: trying to apply these via `docker run --sysctl=...` may be rejected by some runtimes (e.g. Docker Desktop).

```bash
sudo sysctl -w kernel.perf_event_paranoid=1
sudo sysctl -w kernel.kptr_restrict=0
```

To make this persistent across reboots (Ubuntu/Debian example):

```bash
cat <<'EOF' | sudo tee /etc/sysctl.d/99-wrkr-perf.conf
kernel.perf_event_paranoid=1
kernel.kptr_restrict=0
EOF
sudo sysctl --system
```

After changing host settings, rebuild the container.
