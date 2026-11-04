#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path
from unittest.mock import patch


SERVER_PATH = Path(__file__).with_name("server.py")
SPEC = importlib.util.spec_from_file_location("yzw_bio_server", SERVER_PATH)
assert SPEC is not None and SPEC.loader is not None
server = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(server)


class FakeClient:
    def __init__(self, responses: list[object]) -> None:
        self.responses = responses
        self.calls: list[tuple[str, str, dict[str, object]]] = []

    def request(self, method: str, path: str, **kwargs: object) -> object:
        self.calls.append((method, path, kwargs))
        response = self.responses.pop(0)
        if isinstance(response, Exception):
            raise response
        return response


class YzwBioTests(unittest.TestCase):
    def test_mcp_lists_expected_tools(self) -> None:
        response = server._handle({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        names = [tool["name"] for tool in response["result"]["tools"]]
        self.assertEqual(
            names,
            [
                "search_papers",
                "resolve_entity",
                "get_paper",
                "get_paper_annotations",
                "find_related_entities",
                "get_paper_resource",
                "semantic_similarity",
                "service_health",
            ],
        )

    def test_search_prefers_tagp(self) -> None:
        client = FakeClient([{"ok": True, "data": {"items": []}}])
        with patch.object(server, "_client", return_value=client):
            result = server.search_papers({"query": "rice disease resistance"})
        self.assertEqual(result["source"], "tagp")
        self.assertEqual(client.calls[0][1], "/tagp/v1/search")

    def test_search_falls_back_to_graph_v1(self) -> None:
        client = FakeClient(
            [server.ToolError("tagp unavailable"), {"data": [{"paperId": "1"}]}]
        )
        with patch.object(server, "_client", return_value=client):
            result = server.search_papers(
                {"query": "maize drought", "page": 2, "page_size": 5}
            )
        self.assertEqual(result["source"], "graph_v1_fallback")
        self.assertEqual(client.calls[1][1], "/graph/v1/paper/search")
        self.assertEqual(client.calls[1][2]["query"]["offset"], 5)

    def test_resolve_entity_uses_selected_type(self) -> None:
        client = FakeClient([{"ok": True, "data": {"items": []}}])
        with patch.object(server, "_client", return_value=client):
            server.resolve_entity(
                {"entity_type": "species", "query": "Oryza sativa", "limit": 3}
            )
        self.assertEqual(client.calls[0][1], "/tagp/v1/resolve/species")

    def test_get_paper_encodes_doi_path(self) -> None:
        client = FakeClient([{"ok": True, "data": {"id": "x"}}])
        with patch.object(server, "_client", return_value=client):
            server.get_paper({"paper_id": "doi:10.1000/example/test"})
        self.assertEqual(
            client.calls[0][1],
            "/tagp/v1/paper/doi%3A10.1000%2Fexample%2Ftest",
        )

    def test_similarity_omits_raw_embeddings(self) -> None:
        client = FakeClient(
            [
                {
                    "model_name": "bge-m3",
                    "dimension": 2,
                    "embeddings": [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
                }
            ]
        )
        with patch.object(server, "_client", return_value=client):
            result = server.semantic_similarity({"texts": ["a", "b", "c"]})
        self.assertNotIn("embeddings", result)
        self.assertEqual(result["similarity_matrix"][0], [1.0, 0.0, 0.707107])

    def test_tool_errors_are_mcp_results(self) -> None:
        result = server._call_tool("resolve_entity", {"query": "x", "entity_type": "bad"})
        self.assertTrue(result["isError"])
        self.assertIn("entity_type", result["content"][0]["text"])

    def test_tool_catalog_is_json_serializable(self) -> None:
        json.dumps(server.TOOLS, ensure_ascii=False)


if __name__ == "__main__":
    unittest.main()
