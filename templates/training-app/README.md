# Training App Template

Default POC customer template for employee training.

M5 package:

- Manifest: `template.json`.
- Frontend app: `training-web` at `apps/training-web`.
- C-side pages: learning, ask, quiz, records, notifications.
- Smoke checks: `pnpm test` and `pnpm build` in `apps/training-web`.
- Includes training roles, learner menus, quiz and reminder skills, Feishu connector, reminder trigger, and `training_regression` eval set.

## Frontend pages

| Code | Path | Permission |
| --- | --- | --- |
| `learn` | `/` | `app:training:learn` |
| `ask` | `/ask` | `app:training:ask` |
| `quiz` | `/quiz` | `app:training:quiz` |
| `records` | `/records` | `app:training:learn` |
| `notifications` | `/notifications` | `app:training:learn` |

## Smoke checks

Script: `templates/training-app/smoke.sh`

| Code | Workdir | Command |
| --- | --- | --- |
| `training_web_unit` | `apps/training-web` | `pnpm test` |
| `training_web_build` | `apps/training-web` | `pnpm build` |
