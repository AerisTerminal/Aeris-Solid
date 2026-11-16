# Aeris Solid

The next Aeris Terminal: one Solid UI that runs in the browser and as a native
desktop app through Tauri. It replaces the GPUI desktop app
(`Aeris Terminal`, 1.x) once it reaches feature parity.

## Why

- Text and UI rendering at browser quality. GPUI's Windows text path
  (DirectWrite with system gamma, contrast and grid fitting) does not match it.
- One UI for web and desktop. A web version needs no download, which lowers the
  barrier for new users.
- A much larger ecosystem and hiring pool than GPUI.

## Architecture

- **UI:** Solid, with fine-grained updates and no virtual DOM. Panels, menus,
  forms, tables and account views are DOM.
- **Heavy views:** chart, order-book ladder, footprint and tape are drawn by
  Aeris Charts (Rust compiled to WebAssembly, WebGPU with a Canvas 2D fallback,
  OffscreenCanvas in a worker). They never render a DOM node per price level
  or trade.
- **Desktop backend:** the Rust runtimes (`market_runtime`,
  `trading_runtime`, `account_runtime`) run inside Tauri. `market_runtime`
  first moves its provider code out into adapter crates (see Provider
  adapters); the rest of its logic carries over.
- **Web backend:** the same runtimes compiled to WebAssembly in a worker.
  Hyperliquid and practice trading come first. Rithmic on the web only after
  Rithmic approves a browser client.
- **Data path:** runtimes ingest every event. The UI receives at most one
  binary update per frame (Tauri binary channels on desktop, transferable
  buffers from the worker on the web). No JSON and no per-tick messages on the
  hot path, and no allocation per frame.
- **Local-first:** the app connects directly to providers. No Aeris server
  receives, stores or relays market data, orders or account state.
- **Compatibility:** workspaces and trading stores written by Aeris 1.x must
  open and migrate.

## Crypto providers

Both need no account or API key, and both work for US users.

- **Hyperliquid:** perps. `l2Book`, about 20 levels per side.
- **Coinbase (Advanced Trade):** spot, plus US futures. Public endpoint
  `wss://advanced-trade-ws.coinbase.com`; `level2` sends a full book snapshot
  and incremental updates without a JWT (checked against Coinbase's docs on
  2026-11-16). Channels close after 60–90 seconds without updates, so
  subscribe to `heartbeats`. Coinbase offers no order-by-order (L3) data to
  normal accounts.

Order-by-order depth comes from CME futures through Rithmic.

## Provider adapters

Adding a provider means adding one crate and one registration line. No
generic code changes, whatever the asset class (crypto, futures, stocks, FX,
options).

**Split of work:**

- The **runtime** owns everything generic: session lifecycle, reconnect
  backoff, demand leases, generation fencing, history merge, canonical
  candles, order-book state, recording and replay.
- An **adapter** only translates between the provider's wire format and the
  normalized model. It keeps no canonical state and makes no product decision.

**Layout:**

- `crates/provider_api`: the adapter contract. It holds the capability traits,
  normalized events, typed errors and capability declarations. It depends only
  on the instrument and domain types.
- `crates/providers/<name>`: one crate per provider (`hyperliquid`,
  `coinbase`, `rithmic`, ...). It depends on `provider_api` and the shared
  building blocks, never on the runtime or another provider.
- `crates/provider_kit`: the shared building blocks: WebSocket client with
  heartbeats, rate-limit budget, and order-book sync (snapshot plus diffs,
  sequence-gap detection, checksums). Adapters stay mostly translation.
- The composition root keeps the one list of registered providers.

**Contract:**

- **Capabilities are declared, not coded:** asset classes, streams (trades,
  quotes, aggregate depth with its level limit, order levels with an
  instrument cap), history intervals, sign-in method (none, API key, vault
  login), trading venue, rate budget and build targets (native, web). The
  runtime and UI read the declaration; nothing branches on a provider name.
- **Small optional traits:** `Catalog` (search, resolve), `MarketFeed` (demand
  in, events out), `History` (bar and trade pages), `Venue` (orders and
  account reports), `SignIn`. A provider implements only what it offers.
- **One normalized event model:** fixed-point values with explicit scales,
  provider and exchange timestamps in separate fields, provider sequence kept
  apart from local ingestion order.
- **Typed errors:** `Transport`, `Auth`, `RateLimited`, `Refused`, `Schema`,
  `Gap` and so on. The runtime reacts to the kind; the provider's text is
  only shown to the user.
- **Transport is injected:** native WebSockets on desktop, the browser's
  WebSocket on the web, so one adapter compiles for both. A provider that
  cannot run in a browser (Rithmic) declares a native-only target.

**Checks:**

- Every adapter passes one shared conformance suite on sanitized recorded
  fixtures: snapshot, incremental update, sequence gap, reconnect, auth
  failure, rate limit and session close.
- A lint fails if a provider name or provider-specific ID format appears
  outside its own crate and the presentation assets.

**Lessons from 1.x:** provider code lived inside the runtime
(`market_runtime/src/hyperliquid_*.rs`, `rithmic_*.rs`, about 9,250 lines) and
leaked into generic files (`HyperliquidSession` and `RithmicSession` in
`provider_session.rs`, Rithmic error text in `provider_event.rs`,
`rithmic_login.rs` in `market_service`, about 150 provider references in
`market_service/mod.rs`). Adding a provider touched more than a dozen generic
files. The contract above removes that.

## Stack

Every dependency starts at its latest stable release. A pre-release (alpha,
beta, rc, next) is used only by an explicit decision noted here.

Latest stable versions as of 2026-11-16:

| Purpose | Package | Version |
|---|---|---|
| Runtime and package manager | Bun | 1.4.4 |
| Language | TypeScript | 7.0.2 (7+, never 6) |
| UI framework | `solid-js` | 1.9.19 |
| Routing | `@solidjs/router` | 1.0.3 |
| Build | `vite` | 8.3.8 |
| Solid build plugin | `vite-plugin-solid` | 2.11.16 |
| Desktop shell | Tauri (`tauri` crate, `@tauri-apps/cli`, `@tauri-apps/api`) | 2.12 |
| Components | `@kobalte/core` through solid-ui (shadcn port, copied into the repo) | 0.13.14 |
| Styling | `tailwindcss`, `@tailwindcss/vite` | 4.3.5 |
| Tables | `@tanstack/solid-table` | 9.3.1 |
| Virtual lists | `@tanstack/solid-virtual` | 3.13.53 |
| Async data | `@tanstack/solid-query` | 5.107.1 |
| Lint and format | `@biomejs/biome` | 2.6.8 |
| Charts | Aeris Charts (`aeris_charts_wasm`, `packages/charts`) | pinned Git revision |

Pre-releases to watch: Solid 2.0 (rc), Kobalte 2.0 (alpha), Tauri 3 (alpha).
Move to each when it ships stable.

### Why Kobalte

Radix and Base UI are React-only. Kobalte is the most used Solid component
library (about 5.9M npm downloads a week, against about 92K for Ark UI) and
the closest Solid equivalent to Radix: unstyled, accessible, compound
components. Its version is still below 1.0, so pin it exactly and read the
changelog before each upgrade.

### Design

`themes.md` in this folder is the styling source: the Aeris theme and seven
presets, each light and dark, with every token. Colors, radii and borders come
only from its variables, never a hardcoded value or an ad-hoc tint. Themes are
picked with `data-theme` and dark mode with the `dark` class on `<html>`, as
`themes.md` shows.

- The terminal UI uses `--text-default` on both desktop and web.
  `--text-web-default` is only for the website and landing pages.
- Inter is the UI face and Manrope the numeric face with tabular figures.
- Floating layers, dropdown and context menus included, sit on
  `--surface-secondary` with a 1px `--border-secondary` edge. Pages and
  panels use `--surface`.

## Naming

Each language keeps its own convention.

| What | Style | Example |
|---|---|---|
| Files and folders in `src/` | `kebab-case` | `order-book.tsx`, `dropdown-menu.tsx` |
| Files, folders, modules, functions, variables in `src-tauri/` | `snake_case` | `market_bridge.rs`, `place_order` |
| TypeScript variables and functions | `camelCase` | `accountId`, `placeOrder` |
| Solid components, types, interfaces, Rust types | `PascalCase` | `OrderBook`, `MarketSnapshot` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAXIMUM_PANES` |
| CSS classes, CSS variables, URL paths | `kebab-case` | `--surface-backdrop`, `/account/settings` |

- `src/` uses kebab-case because solid-ui and shadcn ship kebab-case
  components; copied components keep their names, so updates diff cleanly.
- Rust module names must be valid identifiers, so `src-tauri/` stays
  `snake_case`.
- Never mix styles within one side. File names stay all lowercase, so
  case-insensitive file systems (Windows, macOS) cannot break imports.
- Tauri converts command arguments between the two sides: Rust
  `place_order(account_id)` is called as `invoke("place_order", { accountId })`.
- Biome's `useFilenamingConvention` rule enforces kebab-case file names in
  `src/`, and a wrong name fails the check.

## Rules

- Stay on the latest stable versions. Upgrade deliberately, one dependency
  family at a time, with the checks passing.
- Commit `bun.lock` and `src-tauri/Cargo.lock`.
- Hot-path views stay in Aeris Charts. Never add a DOM row per tick, price
  level or trade.
- Measure before claiming performance: idle memory, startup time, and frame
  times (worst 1%) while replaying a recorded busy session at 100x, compared
  against the GPUI build.

## First milestone: spike

One Tauri window with an Aeris Charts pane and the order book on live
Hyperliquid data from the existing `market_runtime`. It must show:

1. text and chart sharpness matching the React web app,
2. Tauri binary channels keeping up with live order-book updates,
3. idle memory, startup time and frame times against the GPUI build.

## Development

```
bun install
bun run tauri dev    # desktop
bun run dev          # web
```
