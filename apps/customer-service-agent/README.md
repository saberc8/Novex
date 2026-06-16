# Customer Service Agent App

Minimal smokeable frontend entry for the customer-service agent delivery template.

This package validates the first delivery contract:

- `/customer-service` requires `ai:customer-service:agent:run`.
- `/customer-service/runs` requires `ai:customer-service:agent:list`.
- `/customer-service/knowledge` requires `ai:customer-service:read`.

Run:

```bash
pnpm test
```
