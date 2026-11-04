#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
yzw_home="${YZW_HOME:-$HOME/.yzw}"
yzw_codex_home="${YZW_CODEX_HOME:-$HOME/.yzw-codex}"
qiri_catalog="${QIRI_MODEL_CATALOG:-$HOME/.qiri-codex/models.json}"

install -d -m700 "$yzw_home" "$yzw_codex_home"
install -m755 "$script_dir/yzw" "$HOME/.local/bin/yzw"
install -m755 "$repo_root/tools/yzw-bio/server.py" "$HOME/.local/bin/yzw-bio-mcp"

sed "s|@YZW_CODEX_HOME@|$yzw_codex_home|g" "$script_dir/config.toml" \
    > "$yzw_codex_home/config.toml"
chmod 600 "$yzw_codex_home/config.toml"

if [[ ! -r "$qiri_catalog" ]]; then
    printf 'QIRI model catalog is required: %s\n' "$qiri_catalog" >&2
    exit 1
fi

python3 - "$qiri_catalog" "$yzw_codex_home/models.json" <<'PY'
import json
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
catalog = json.loads(source.read_text(encoding="utf-8"))
for model in catalog.get("models", []):
    description = model.get("description")
    if isinstance(description, str):
        model["description"] = description.replace(
            "QIRI (AI-for-Science research agent)",
            "YZW (Yazhou Bay agricultural breeding research agent)",
        ).replace("QIRI", "YZW")
    messages = model.get("model_messages") or {}
    instructions = messages.get("instructions_template")
    if isinstance(instructions, str):
        messages["instructions_template"] = instructions.replace(
            "穹理 QIRI", "崖州湾 YZW"
        ).replace("QIRI", "YZW")
target.write_text(json.dumps(catalog, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
target.chmod(0o600)
PY

if [[ ! -e "$yzw_home/.env" ]]; then
    printf '%s\n' \
        'YZW_SOCKS_PROXY="127.0.0.1:1080"' \
        'YZW_DATA_API_KEY_FILE="$HOME/.yzw/api.txt"' \
        > "$yzw_home/.env"
    chmod 600 "$yzw_home/.env"
fi

if [[ -n "${YZW_DATA_API_KEY_SOURCE:-}" ]]; then
    install -m600 "$YZW_DATA_API_KEY_SOURCE" "$yzw_home/api.txt"
fi

if [[ ! -s "$yzw_home/api.txt" ]]; then
    printf 'Install completed, but the data API key is missing.\n' >&2
    printf 'Create %s or rerun with YZW_DATA_API_KEY_SOURCE=/path/to/api.txt.\n' "$yzw_home/api.txt" >&2
fi

printf 'Installed YZW launcher and MCP tools. Start the SOCKS5 tunnel, then run: yzw\n'
