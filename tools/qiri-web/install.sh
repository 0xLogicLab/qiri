#!/usr/bin/env bash
set -euo pipefail
qiri_tools_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
qiri_install_dir="$HOME/.local/lib/qiri"
mkdir -p "$qiri_install_dir" "$HOME/.local/bin"
python3 -m venv "$qiri_install_dir/web-venv"
"$qiri_install_dir/web-venv/bin/python" -m pip install -r "$qiri_tools_dir/qiri-web/requirements.txt"
install -m755 "$qiri_tools_dir/qiri-literature/server.py" "$qiri_install_dir/web-server.py"
install -m755 "$qiri_tools_dir/qiri-web/qiri-web-mcp" "$HOME/.local/bin/qiri-web-mcp"
echo 'Installed qiri-web-mcp. Configure mcp_servers.qiri_web using tools/qiri-launcher/config.toml.'
