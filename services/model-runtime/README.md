# Model Runtime

Optional sidecar for private model adapters and ML protocol normalization.

Allowed responsibilities:

- Expose private LLM, embedding, rerank, or VLM services through a stable adapter.
- Normalize local model endpoints into OpenAI-compatible or Novex-controlled contracts.
- Probe model health when requested by the Rust control plane.

Boundaries:

- Model registration, routing, credentials, policy, usage, and audit stay in Rust.
- This service does not select models for tenants.
- This service is optional for POC deployments that use external APIs.

M0 status: directory skeleton only.
