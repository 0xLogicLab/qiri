import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


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

    def test_stdio_protocol_and_catalog_isolation(self):
        for args, expected in [
            (["--web"], ["web_search", "fetch_url"]),
            ([], ["search_literature", "fetch_url"]),
        ]:
            requests = [
                {"jsonrpc": "2.0", "id": 1, "method": "initialize"},
                {"jsonrpc": "2.0", "method": "notifications/initialized"},
                {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
                {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
                    "name": "web_search" if not args else "search_literature",
                    "arguments": {"query": "test"},
                }},
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

    def test_search_normalizes_and_deduplicates_real_provider_shape(self):
        client = Mock()
        client.text.return_value = [
            {"title": "A", "href": "https://example.org/a", "body": "Excerpt"},
            {"title": "A duplicate", "href": "https://example.org/a", "body": "Other"},
            {"title": "Bad", "href": "javascript:alert(1)", "body": "Bad"},
            {"title": "B", "href": "https://example.org/b", "body": "Second"},
        ]
        factory = Mock(return_value=client)
        with patch.dict(sys.modules, {"ddgs": SimpleNamespace(DDGS=factory)}), patch.dict(
            os.environ, {"DDGS_PROXY": "http://proxy.test:1234"},
        ):
            response = self.server._call_tool("web_search", {
                "query": " test ", "limit": 2, "region": "cn-zh", "timelimit": "w",
            })
        result = response["structuredContent"]
        self.assertEqual(result["count"], 2)
        self.assertEqual(result["results"][0], {"title": "A", "url": "https://example.org/a", "snippet": "Excerpt"})
        self.assertIn("+00:00", result["retrieved_at"])
        self.assertEqual(client.text.call_args.kwargs["timelimit"], "w")
        self.assertEqual(client.text.call_args.args, ("test",))
        self.assertEqual(factory.call_args.kwargs["proxy"], "http://proxy.test:1234")

    def test_bad_arguments_are_reported_without_network(self):
        for args in [
            {}, {"query": " "}, {"query": 123}, {"query": "x", "limit": True},
            {"query": "x", "limit": 21}, {"query": "x", "region": "bad"},
            {"query": "x", "timelimit": "hour"}, {"query": "x", "safesearch": "bad"},
        ]:
            self.assertTrue(self.server._call_tool("web_search", args)["isError"], args)

    def test_provider_failure_is_not_a_successful_empty_result(self):
        for value in [[], RuntimeError("upstream unavailable")]:
            client = Mock()
            if isinstance(value, Exception):
                client.text.side_effect = value
            else:
                client.text.return_value = value
            with patch.dict(sys.modules, {"ddgs": SimpleNamespace(DDGS=Mock(return_value=client))}):
                response = self.server._call_tool("web_search", {"query": "test"})
            self.assertTrue(response["isError"])
            self.assertNotIn("structuredContent", response)

    def test_fetch_rejects_private_urls_before_opening(self):
        with patch.object(self.server.socket, "getaddrinfo", return_value=[
            (2, 1, 6, "", ("127.0.0.1", 443)),
        ]), patch.object(self.server, "build_opener") as opener:
            response = self.server._call_tool("fetch_url", {"url": "https://internal.test"})
        self.assertTrue(response["isError"])
        opener.assert_not_called()


if __name__ == "__main__":
    unittest.main()
