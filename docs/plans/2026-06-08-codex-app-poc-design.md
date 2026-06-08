# Codex App POC Design

## Goal

Build a standalone web POC that closely reproduces the Codex desktop app workbench feel without using OpenAI or Codex brand assets. The first screen is a usable developer agent workspace, not a marketing page.

## Product Shape

The app lives at `apps/codex-app-poc` and is independent from the existing `agent-workspace`. It uses a fixed left sidebar and a white main work area. The sidebar presents new chat, search, plugins, automations, pinned projects, projects, current-project conversations, and settings. The main area presents the empty-session composer and command menu states.

The POC is intentionally front-end only. It uses static data for projects, sessions, plugins, automations, command items, and suggestions. It should feel interactive enough for demos: input text changes local state, typing `/` opens the command menu, arrow keys move the active command, Enter selects, and Esc closes the menu.

## Visual Design

The base background is a warm gray (`#F3ECEC`) and the primary panel is white. The composition should feel like a macOS desktop app: restrained spacing, subtle radii, quiet borders, system fonts, and lucide-style line icons. It avoids hero sections, gradients, decorative illustrations, product logos, and card-heavy marketing composition.

Primary colors:

- Background: `#F3ECEC`
- Main panel: `#FFFFFF`
- Text: `#111111`
- Secondary text: `#8A8A8A`
- Border: `#E5E5E5`
- Selected background: `#F1F1F1`
- Accent: `#F97316`
- Send button: `#050505`

## Layout

The app targets desktop web sizes first: `1440x900`, `1728x1117`, and `2048x1280`. The sidebar is about 300px wide on desktop. The main work area has a white panel with a slight top-left radius and fills the remaining viewport. The empty session content is centered horizontally and positioned slightly above the vertical midpoint, with a maximum width around 860px.

On smaller viewports the sidebar narrows and lower-priority labels may wrap, but core elements must not overlap. The POC only needs basic responsive support for web review.

## Components

The initial implementation should create:

- `CodexPocApp`: owns local UI state and page composition.
- `Sidebar`: renders top controls, navigation, pinned/project/session groups, and settings.
- `Composer`: renders the large input, toolbar, current directory row, and suggestions.
- `CommandMenu`: renders the `/` menu and keyboard selection state.
- Small local data arrays for nav items, projects, sessions, command items, and suggestions.

## Interactions

Typing in the composer is local only. When the composer contains `/`, the command menu appears above the composer. Arrow up/down cycles command selection. Enter copies the selected command label into the composer and closes the menu. Esc closes the menu.

The send button does not call a backend. It can append a lightweight local message preview later, but the first POC only needs the empty state and composer behaviors.

## Testing

Use React Testing Library and Vitest for behavior tests:

- The home workbench renders core Chinese labels and sidebar groups.
- The composer renders attachment, permission, directory, model, reasoning, and send controls.
- Typing `/` opens the command menu.
- Arrow keys move selection and Esc closes the menu.

Use `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `pnpm build` from `apps/codex-app-poc` for verification.
