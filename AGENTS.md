# AGENTS.md

Rules for every change to Aeris Solid, by a person or a coding agent. Read
`README.md` (plan, stack, providers, naming) and `themes.md` (styling tokens)
first. These rules are acceptance criteria: a change that breaks one is not
done, and an existing violation is never a reason to add another.

## Layout

```
crates/
  domain/            instruments, fixed-point prices, time, IDs, the market feed contract. No I/O.
  provider_api/      adapter contract: capability traits, normalized events, typed errors
  provider_kit/      shared adapter parts: WebSocket, rate budget, order-book sync
  providers/<name>/  one crate per provider
  market_runtime/    market owner: sessions, demand, history, candles, order books, recording
  trading_runtime/   trading owner: accounts, orders, fills, positions, P&L, local store
  account_runtime/   sign-in session and profile
  app_protocol/      every message between backend and UI; generates the TypeScript types
  app_core/          composition: builds the runtimes, registers providers, routes UI
                     commands, batches per-frame updates
  web_host/          web shell: app_core compiled to WebAssembly in a worker. No logic.
src-tauri/           desktop shell: Tauri commands and channels over app_core. No logic.
src/                 Solid UI
vendor/aeris-charts/ pinned Aeris Charts checkout (REVISION records the commit).
                     The chart engine the heavy views draw through; never forked
                     or edited here.
tools/arch_check/    layer and naming checks
```

Each piece of behavior has one home in this tree. If it has no obvious home,
decide where it belongs and record it here before writing it.

## Rust dependency direction

| Crate | May depend on |
|---|---|
| `domain` | no workspace crate |
| `provider_api` | `domain` |
| `provider_kit` | `domain`, `provider_api` |
| `providers/*` | `domain`, `provider_api`, `provider_kit`. Never another provider or a runtime |
| `market_runtime` | `domain`, `provider_api`. Never a provider crate: providers are registered by `app_core` |
| `trading_runtime` | `domain`. Never `market_runtime`: it reads the market feed contract in `domain` |
| `account_runtime` | `domain` |
| `app_protocol` | `domain` |
| `app_core` | everything above. The only crate that lists providers |
| `src-tauri`, `web_host` | `app_core`, `app_protocol` |

## UI layout and imports

```
src/
  app/              shell, routes, layout. Composes features; holds no feature logic.
  features/<name>/  one folder per feature: chart, order-book, ticket, positions,
                    accounts, workflows, screener, settings, ...
  ui/               generic components (solid-ui copies). No feature or backend knowledge.
  bridge/           the only code that talks to the backend. `tauri.ts` and `worker.ts`
                    implement one interface.
  protocol/         TypeScript types generated from `app_protocol`. Never edited by hand.
  lib/              pure helpers (formatting, math). No Solid, no backend.
  styles/           `themes.md` tokens and global CSS
```

- `ui/` and `lib/` never import `features/`, `bridge/` or `app/`.
- A feature imports `ui/`, `lib/`, `protocol/` and `bridge/`, never another
  feature. Code two features need moves to `ui/` or `lib/`; wiring between
  features belongs in `app/`.
- Only `bridge/` imports `@tauri-apps/api` or posts to the worker.

## Ownership

- The backend owns all canonical state: market data, orders, positions,
  accounts and the saved workspace. The UI keeps view state only (open menus,
  scroll, hover, unsaved form input).
- The UI displays the runtime's values. It never computes P&L, fills, candles,
  order-book state or anything else a runtime owns.
- One owner per piece of state. No copies kept in sync by hand.
- Commands carry intent (instrument, side, quantity, order type), never an
  authoritative price.
- The chart, footprint and tape are drawn by Aeris Charts. Feature code feeds
  them data and handles their events. The order book is a platform view owned
  by `features/order-book`: canvas-rendered, fed by the runtime's book stream,
  never a DOM row per price level. Aeris Charts' depth indicators are chart
  studies, not the platform order book.
- The workspace has one writer, in the backend. The UI sends section updates.
- Provider rules (adapters translate only, capabilities are declared) are in
  `README.md` under Provider adapters.

## Data path

- UI to backend: typed commands from `app_protocol`, each answered with a typed
  result or error.
- Backend to UI: at most one binary update per stream per frame, as snapshot
  plus deltas. A consumer that falls behind receives a fresh snapshot.
- No JSON and no per-tick message on the hot path.

## Bounds and errors

- Every queue, cache, list and retry is bounded, with explicit behavior when
  full. An unbounded collection in production code is a bug.
- Errors are typed at every boundary. Never branch on display text.
- Rust: no `unwrap` or `expect` in production code outside startup invariants.
  TypeScript: `strict` mode, no `any`.

## Checks

- `tools/arch_check` reads every `Cargo.toml` and fails on a dependency the
  table above forbids. It also fails on a provider name outside
  `crates/providers/<name>` and the provider artwork.
- dependency-cruiser enforces the UI import rules. Biome lints, formats and
  enforces kebab-case file names in `src/`.
- Before every commit: `bun run check` (Biome, TypeScript, dependency-cruiser),
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `cargo run -p arch_check`.
- A rule changes only by editing this file and its check in the same commit.
  Until a check exists, review enforces the rule; adding the checks is part of
  the first milestone.

## Working rules

- Work on `main`. No force push, no `reset --hard`, no deleting work you did not
  make. Commit only after the checks pass, in `type(scope): outcome` style, with
  no agent or tool attribution.
- Dependencies start at the latest stable version (see `README.md`).
- Styling comes only from `themes.md` tokens: no hardcoded colors, radii or
  borders.
- Edit source with file tools so every change is a reviewable diff. No scripts
  that rewrite source files.
- Never put credentials, account data or the licensed `rithmic_proto_kit` in
  the repository, logs or a hosted AI model's context.
- Durable state written by any released version keeps opening. Aeris 1.x
  workspaces and trading stores migrate.
