# Training App Template

Default POC customer template for employee training.

M5 package:

- Manifest: `template.json`.
- Frontend app: `training-web` at `apps/training-web`.
- C-side pages: learning, ask, quiz, records, notifications.
- Smoke checks: `pnpm test` and `pnpm build` in `apps/training-web`.
- Includes training roles, learner menus, quiz and reminder skills, Feishu connector, reminder trigger, and `training_regression` eval set.
