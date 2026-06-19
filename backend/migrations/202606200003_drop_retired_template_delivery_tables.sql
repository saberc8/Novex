-- Drop retired template-delivery tables that are no longer wired to runtime services.
-- Historical migrations stay in place for SQLx version continuity; this migration
-- makes existing and fresh databases converge on the unified Admin delivery model.

DROP TABLE IF EXISTS ai_template_smoke_result;
DROP TABLE IF EXISTS ai_template_smoke_run;
DROP TABLE IF EXISTS ai_customer_frontend_config;
DROP TABLE IF EXISTS ai_customer_package;
