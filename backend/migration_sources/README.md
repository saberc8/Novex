# Backend Migration Sources

`backend/migration_sources` is the human-oriented source tree for database migrations.
`backend/migrations` remains the flat SQLx execution directory used by `sqlx migrate run`
and `sqlx::migrate!("./migrations")`.

## Layout

Source migrations live under:

```text
backend/migration_sources/<module>/<kind>/<version>_<description>.sql
```

`<kind>` is derived from the filename:

- `schema`: `create_*`
- `seed`: `seed_*`
- `patch`: `add_*`, `patch_*`, `remove_*`, `sanitize_*`, `enrich_*`, `promote_*`, or other non-create/non-seed changes

Examples:

```text
backend/migration_sources/system/schema/202605290001_create_sys_core.sql
backend/migration_sources/ai/model/seed/202606200002_seed_ai_model_manage_permission.sql
backend/migration_sources/ai/template/patch/202606200001_remove_ai_template_menu.sql
```

Historical migrations may bundle schema and bootstrap data when the original migration
already did so. New migrations should prefer one responsibility per file: schema, seed,
or patch.

## Workflow

1. Add or edit the source SQL under `backend/migration_sources`.
2. Run `scripts/sync-migration-sources.sh` from the repository root.
3. Run `scripts/sync-migration-sources.sh --check`.
4. Run `cargo test -p backend migration_sources_mirror_sqlx_flat_directory --offline`.

Do not hand-edit `backend/migrations` unless you are repairing a drift check failure.
