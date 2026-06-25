# Research Radar i18n Design

## Goal

Support Chinese and English in the Research Radar POC, with Chinese as the default language.

The first pass should make the product usable for Chinese users without losing the existing English demo value. The UI should start in Chinese, allow switching to English, remember the user's choice, and keep source/API behavior unchanged.

## Decisions

- Default language is `zh-CN`.
- Supported languages are `zh-CN` and `en-US`.
- If the user has selected a language before, load it from local storage.
- If no saved choice exists, use `zh-CN`. Do not auto-switch based on browser language in this pass.
- The selected language affects UI copy and the Agent report language instruction.
- Source results, paper titles, project names, URLs, author names, and dataset names remain in their original source language.

## Scope

In scope:

- Add a language selector in the Research Radar header.
- Translate visible Research Radar POC UI labels, helper text, empty states, error messages, graph labels, drawer labels, inspector labels, and status labels.
- Persist language choice in local storage.
- Pass the selected language into `createResearchRadarRun`.
- Update Agent prompt instructions so model reports are requested in the selected language.
- Add tests for default Chinese, English switching, persistence, and prompt language instruction.

Out of scope:

- Full app-wide internationalization outside `apps/research-radar-poc`.
- Backend locale negotiation.
- Translating third-party source content.
- Browser-language auto-detection.
- Runtime translation of existing model output after it has already been generated.

## UX

The header gets a compact language selector near the model selector:

- `中文`
- `English`

The default visible UI uses Chinese copy, including:

- App title remains `Research Radar` for product identity.
- Input label: `研究主题`.
- Primary action: `启动雷达扫描`.
- Evidence rail title: `证据`.
- Node inspector title: `节点详情`.
- Evidence drawer title: `证据抽屉`.
- Empty and error states use Chinese copy.

When switched to English, the same controls use English copy. The toggle should not reset the current scan, graph selection, filters, or topic input.

## Architecture

Add a small frontend i18n layer rather than introducing a large dependency.

New or updated pieces:

- `src/lib/i18n.ts`
  - Defines `ResearchLocale = "zh-CN" | "en-US"`.
  - Exports locale labels.
  - Exports a typed translation dictionary.
  - Exports helpers for reading/writing local storage safely.
- `src/app-client.tsx`
  - Owns `locale` state.
  - Renders the language selector.
  - Uses dictionary strings for UI copy.
  - Passes `locale` into API calls.
- `src/api/research.ts`
  - Extends `ResearchScanInput` handling so prompt generation can request Chinese or English reports.
  - Keeps total Agent input under the existing 4000 character cap.

The dictionary should be explicit and typed. Missing translation keys should be caught at compile time.

## Data Flow

1. App starts.
2. Read saved locale from local storage.
3. If saved locale is valid, use it; otherwise use `zh-CN`.
4. User can switch language from the header.
5. Save the new locale to local storage.
6. UI re-renders using the selected dictionary.
7. On scan submit, pass the selected locale into the research run request.
8. Agent prompt asks for the report in the selected language.

## Error Handling

- If local storage is unavailable, keep the in-memory locale and continue.
- If local storage contains an unsupported value, ignore it and use `zh-CN`.
- If a translation value is missing during development, TypeScript should fail through typed dictionaries.
- If the Agent ignores the requested language, the UI still remains in the selected language; no post-processing translation is attempted.

## Testing

Add focused tests:

- Initial page defaults to Chinese.
- Switching to English updates visible UI labels.
- Saved `en-US` locale is restored on load.
- Invalid saved locale falls back to Chinese.
- Agent prompt includes a Chinese report-language instruction for `zh-CN`.
- Agent prompt includes an English report-language instruction for `en-US`.
- Existing 4000 character cap test still passes.

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx src/api/research.test.ts
pnpm --dir apps/research-radar-poc typecheck
pnpm --dir apps/research-radar-poc lint
```

## Acceptance Criteria

- Research Radar opens in Chinese by default.
- User can switch between Chinese and English without losing current page state.
- Language choice persists across reloads.
- Agent report prompt language follows the selected language.
- Source scanning behavior and external source integrations do not change.
- Existing graph-first functionality keeps working.
