# Responses API proxy

Forwards `POST /v1/responses` to an explicitly configured local or self-hosted
Responses endpoint. It reads the upstream API key from stdin and listens on
loopback. All other routes return 403, except optional `GET /shutdown`.

```sh
printenv QIRI_MODEL_API_KEY | codex-responses-api-proxy \
  --upstream-url http://127.0.0.1:8000/v1/responses \
  --port 60001
```

Configure Qiri with `base_url = "http://127.0.0.1:60001/v1"` and
`wire_api = "responses"` under a custom model provider.

The proxy replaces Authorization and Host headers, preserves streaming response
bodies and status codes, and refuses redirects. API keys use the existing locked
memory storage. There is no built-in cloud endpoint or federated token exchange.

Optional flags: `--server-info <FILE>` writes the port and pid as JSON;
`--dump-dir <DIR>` records exchanges (headers with authorization/cookies are
redacted, bodies may contain sensitive data); `--http-shutdown` enables shutdown.
