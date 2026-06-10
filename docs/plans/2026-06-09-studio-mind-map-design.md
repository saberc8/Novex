# Studio Mind Map Design

## Goal

Build "generate mind map" as the first Novex Studio Artifact capability. The feature must strengthen the agent foundation instead of becoming a one-off right-panel action.

## Architecture

Novex should introduce a reusable Studio Artifact runtime between knowledge RAG and customer-facing UI. A Studio Action declares an available operation such as `mind_map.generate`. A Studio Artifact stores the generated result, citations, source snapshot, run linkage, and rendering metadata. Plugins may declare Studio Actions, but plugins do not directly own artifact persistence or bypass RBAC.

```text
Knowledge Dataset
  -> Studio Action: mind_map.generate
    -> Studio runtime
      -> RAG retrieval
      -> model generation
      -> JSON schema validation
      -> artifact persistence
    -> Studio Artifact: mind_map
      -> chat-web renderer
```

## Peer Project Lessons

- Dify shows that long-running AI work needs event snapshots and recoverable runtime state.
- FastGPT shows that knowledge retrieval, AI generation, and tools should be composable capabilities, but a full visual workflow builder is not required for this slice.
- Codex shows that tool-like operations need structured calls, IDs, traces, and raw payload references.
- Hive shows that generated outputs should be first-class artifacts, not only text inside a message log.

## Backend Design

Add a Studio module under the existing AI HTTP boundary:

- `GET /ai/studio/actions`
- `GET /ai/knowledge/datasets/:dataset_id/artifacts`
- `POST /ai/knowledge/datasets/:dataset_id/artifacts/generate`
- `GET /ai/studio/artifacts/:artifact_id`

The first action is seeded as:

- code: `mind_map.generate`
- artifact type: `mind_map`
- surface: `knowledge`
- permission: `ai:studio:artifact:create`
- model purpose: `artifact_generate`, falling back to `rag_answer`

The runtime should persist a structured artifact with JSON content, citations, dataset/session context, action code, status, and trace references. It should be ready to attach `ai_run` in a later hardening pass; the first slice stores a `run_id` field and returns it as nullable until the shared run API is made available to Studio.

## Mind Map Schema

```json
{
  "title": "string",
  "nodes": [
    {
      "id": "root",
      "label": "Topic",
      "summary": "Short explanation",
      "children": ["child-1"],
      "citationRefs": ["c1"]
    }
  ],
  "edges": [
    { "from": "root", "to": "child-1", "label": "contains" }
  ],
  "citations": [
    {
      "id": "c1",
      "documentId": "123",
      "chunkId": "123:0",
      "pageNo": 2,
      "sectionPath": ["Section"]
    }
  ]
}
```

## Frontend Design

`apps/chat-web` should stop using hard-coded Studio cards for mind map generation. The right panel loads available actions, renders `mind_map.generate`, calls artifact generation, and displays the latest mind map artifact. Rendering uses a compact tree view in MVP and keeps the artifact JSON contract independent of the renderer.

## Error Handling

- Missing Studio permission returns `Forbidden`.
- Unknown or disabled action returns a bad request.
- Dataset access still goes through tenant and dataset existence checks.
- Empty datasets return a user-readable artifact with no nodes instead of failing after model call.
- Invalid model JSON falls back to deterministic local mind-map content built from retrieved citations.

## Testing

- Backend tests assert migration tables, permissions, seeded action, request normalization, and missing-permission rejection.
- Backend service tests cover deterministic mind-map fallback from a RAG answer/citation set.
- Frontend API tests assert correct Studio endpoints and bearer auth.
- Frontend render tests assert the Studio panel loads actions, generates a mind map, and renders nodes.

