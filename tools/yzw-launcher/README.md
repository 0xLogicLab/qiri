# YZW Launcher

The `yzw` launcher reuses the shared QIRI core while loading a separate
`~/.yzw-codex` configuration and the Yazhou Bay domain MCP tools. This keeps
the domain branch thin: future shared engine changes can be merged from
`main`, while agricultural prompts and tools remain under `tools/yzw-*`.

Install on the current machine:

```bash
YZW_DATA_API_KEY_SOURCE=/home/sig/files/qiri-codex/config/yzw.txt tools/yzw-launcher/install.sh
```

Before running `yzw`, keep the center51 SOCKS5 proxy available at
`127.0.0.1:1080`. Runtime settings live in `~/.yzw/.env`; model credentials
continue to come from `~/.qiri/.env`.
