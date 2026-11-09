import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import MagicMock, Mock, patch
from urllib.error import HTTPError, URLError

SERVER = Path(__file__).resolve().parents[1] / "qiri-literature" / "server.py"


def load_server():
    spec = importlib.util.spec_from_file_location("qiri_server", SERVER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WebToolsTests(unittest.TestCase):
    def setUp(self):
        self.server = load_server()
        self.server.configure_web_mode()
        self.env = patch.dict(os.environ, {"KEENABLE_API_KEY": "keen_test_secret"})
        self.env.start()
        self.addCleanup(self.env.stop)

    def opener(self, value):
        opener = MagicMock()
        opener.open.return_value.__enter__.return_value.read.return_value = json.dumps(value).encode()
        return opener

    def test_stdio_protocol_and_catalog_isolation(self):
        for args, expected in [(["--web"], ["web_search", "fetch_url"]),
                               ([], ["search_literature", "fetch_url"])]:
            requests = [
                {"jsonrpc": "2.0", "id": 1, "method": "initialize"},
                {"jsonrpc": "2.0", "method": "notifications/initialized"},
                {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
                {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
                    "name": "web_search" if not args else "search_literature",
                    "arguments": {"query": "test"}}},
                {"jsonrpc": "2.0", "id": 4, "method": "ping"},
            ]
            process = subprocess.run(
                [sys.executable, str(SERVER), *args],
                input="\n".join(json.dumps(r) for r in requests) + "\n",
                text=True, capture_output=True, check=True, timeout=5,
            )
            responses = [json.loads(line) for line in process.stdout.splitlines()]
            self.assertEqual(len(responses), 4)
            self.assertEqual(responses[0]["result"]["serverInfo"]["name"], "qiri-web" if args else "qiri-literature")
            self.assertEqual([t["name"] for t in responses[1]["result"]["tools"]], expected)
            self.assertTrue(responses[2]["result"]["isError"])
            self.assertEqual(responses[3]["result"], {})

    def test_authenticated_request_and_normalization(self):
        opener = self.opener({"mode": "realtime", "results": [
            {"title": "A", "url": "https://example.org/a", "snippet": "Excerpt", "published_at": "2026-11-09T00:00:00Z"},
            {"title": "duplicate", "url": "https://example.org/a"},
            {"url": "javascript:alert(1)"}, {"url": "https://user:password@example.org/"},
            {"url": "https://[broken"}, {"url": None}, None,
            {"title": "B", "url": "https://example.org/b", "description": "deprecated"},
        ]})
        with patch.object(self.server, "build_opener", return_value=opener):
            response = self.server._call_tool("web_search", {
                "query": " test ", "limit": 2, "site": " example.org ", "timelimit": "w"})
        result = response["structuredContent"]
        self.assertEqual(result["provider"], "keenable")
        self.assertEqual(result["count"], 2)
        self.assertEqual(result["results"][0]["snippet"], "Excerpt")
        self.assertEqual(result["results"][0]["published_at"], "2026-11-09T00:00:00Z")
        self.assertEqual(result["results"][1]["snippet"], "")
        self.assertIn("+00:00", result["retrieved_at"])
        request = opener.open.call_args.args[0]
        self.assertEqual(request.full_url, "https://api.keenable.ai/v1/search")
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(request.get_header("X-api-key"), "keen_test_secret")
        self.assertEqual(json.loads(request.data), {
            "query": "test", "max_results": 2, "mode": "realtime", "site": "example.org",
            "published_after": "7d", "snippet_max_length": 3000, "disable_cards": True})
        self.assertEqual(opener.open.call_args.kwargs["timeout"], 30)

    def test_publication_windows_and_legacy_options(self):
        for window, delta in [("d", "1d"), ("w", "7d"), ("m", "1mo"), ("y", "1y")]:
            with patch.object(self.server, "_keenable_search", return_value={"results": [{"url": "https://example.org"}]}) as search:
                result = self.server.web_search({"query": "test", "timelimit": window, "region": "cn-zh", "safesearch": "off"})
            self.assertEqual(search.call_args.args[0]["published_after"], delta)
            self.assertNotIn("region", search.call_args.args[0])
            self.assertEqual(result["unsupported_parameters"], ["region", "safesearch"])
        properties = self.server.WEB_SEARCH_TOOL["inputSchema"]["properties"]
        self.assertNotIn("region", properties)
        self.assertNotIn("safesearch", properties)

    def test_missing_key_does_not_call_public_api(self):
        with patch.dict(os.environ, {"KEENABLE_API_KEY": ""}), patch.object(self.server, "build_opener") as opener:
            response = self.server._call_tool("web_search", {"query": "test"})
        self.assertIn("KEENABLE_API_KEY", response["content"][0]["text"])
        opener.assert_not_called()

    def test_bad_arguments_are_reported_without_network(self):
        for args in [{}, {"query": " "}, {"query": 123}, {"query": "x", "limit": True},
                     {"query": "x", "limit": 21}, {"query": "x", "region": "bad"},
                     {"query": "x", "timelimit": "hour"}, {"query": "x", "safesearch": "bad"},
                     {"query": "x", "site": " "}, {"query": "x", "mode": "auto"}]:
            with patch.object(self.server, "build_opener") as opener:
                self.assertTrue(self.server._call_tool("web_search", args)["isError"], args)
                opener.assert_not_called()

    def test_http_errors_are_actionable_and_redact_credentials(self):
        for status, hint in [(401, "KEENABLE_API_KEY"), (402, "credits"), (403, "permissions"),
                             (429, "Retry-After: 5"), (503, "503")]:
            error = HTTPError("https://api.keenable.ai/v1/search", status, "error", {"Retry-After": "5"},
                              io.BytesIO(json.dumps({"code": "test_code", "message": "failure keen_test_secret"}).encode()))
            opener = Mock()
            opener.open.side_effect = error
            with patch.object(self.server, "build_opener", return_value=opener):
                response = self.server._call_tool("web_search", {"query": "test"})
            self.assertTrue(response["isError"])
            text = response["content"][0]["text"]
            self.assertIn(hint, text)
            self.assertIn("test_code", text)
            self.assertNotIn("keen_test_secret", text)
            self.assertEqual(opener.open.call_count, 1)

    def test_network_errors_are_distinct_from_upstream_rejections(self):
        for error, hint in [(TimeoutError(), "timed out"), (URLError(TimeoutError()), "timed out"),
                            (URLError("DNS failed"), "DNS, TLS")]:
            opener = Mock()
            opener.open.side_effect = error
            with patch.object(self.server, "build_opener", return_value=opener):
                response = self.server._call_tool("web_search", {"query": "test"})
            self.assertTrue(response["isError"])
            self.assertIn(hint, response["content"][0]["text"])

    def test_invalid_and_empty_provider_responses_are_errors(self):
        for value in [{"results": []}, {"results": [{"url": "file:///etc/passwd"}]}, {}, [], {"results": "bad"}]:
            with patch.object(self.server, "build_opener", return_value=self.opener(value)):
                response = self.server._call_tool("web_search", {"query": "test"})
            self.assertTrue(response["isError"])
            self.assertNotIn("structuredContent", response)
        opener = self.opener({})
        opener.open.return_value.__enter__.return_value.read.return_value = b"not json"
        with patch.object(self.server, "build_opener", return_value=opener):
            self.assertTrue(self.server._call_tool("web_search", {"query": "test"})["isError"])

    def test_api_redirect_does_not_forward_key(self):
        handler = self.server._KeenableRedirectHandler()
        with self.assertRaises(self.server.ToolError):
            handler.redirect_request(None, None, 302, "Found", {}, "https://other.test")

    def test_fetch_rejects_private_urls_before_opening(self):
        with patch.object(self.server.socket, "getaddrinfo", return_value=[
            (2, 1, 6, "", ("127.0.0.1", 443))]), patch.object(self.server, "build_opener") as opener:
            response = self.server._call_tool("fetch_url", {"url": "https://internal.test"})
        self.assertTrue(response["isError"])
        opener.assert_not_called()


if __name__ == "__main__":
    unittest.main()
