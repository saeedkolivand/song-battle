# Song Battle

A desktop app for **Kick.com** streamers to run music tournaments where **chat votes**
decide the winners, with a transparent **OBS overlay** showing the live matchup.

Built with **Tauri v2 · React 19 · TypeScript · Vite · Tailwind v4 · Zustand · Framer Motion · Rust**.

> Status: **Phase 0 (skeleton)** — the three-runtime pipeline is wired end to end.
> Battle/voting/bracket features land in later phases (see `.claude/plans` / the build plan).

## Architecture

Three runtimes, one source of truth:

- **Dashboard** — React in the Tauri WebView. Talks to Rust over Tauri IPC (`invoke`).
- **Overlay** — React loaded by OBS's embedded Chromium over `http://localhost`. **No Tauri APIs** —
  it's a dumb projection of server snapshots received over a WebSocket.
- **Rust backend** — owns all state (`AppState`). An embedded **axum** HTTP+WebSocket server serves
  the overlay bundle and broadcasts the same snapshot to every overlay client. Kick chat is read in
  Rust and tallied directly into the source of truth.

```
Kick chat (Rust) ─► tally ─► AppState (truth) ─┬─ Tauri events ──► Dashboard
Dashboard ── invoke() ──────────────────────────┘                 axum WS ──► Overlay (OBS)
```

## Repository layout (pnpm + turbo monorepo)

```
apps/desktop    Tauri app: React dashboard (src/) + Rust backend (src-tauri/)
apps/overlay    OBS overlay (separate Vite app; built bundle is embedded in the Rust binary)
packages/types  @sb/types  — shared DTOs mirrored from Rust serde
packages/shared @sb/shared — overlay WS client, provider registry, logger
packages/ui     @sb/ui     — runtime-agnostic presentational components
```

## Prerequisites

- Node 22+ and **pnpm 11**
- **Rust** stable + the [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)
  (on Windows: WebView2, already present on Win11)

## Development

```bash
pnpm install
pnpm --filter @sb/overlay build   # build the overlay once (it is embedded at Rust compile time)
pnpm desktop                      # = pnpm --filter @sb/desktop tauri dev
```

The dashboard shows the Rust IPC result and the overlay URL. While iterating on the overlay UI,
run `pnpm --filter @sb/overlay dev` (port 5174) and point a browser there; the live data still
comes from the axum WebSocket on the desktop app's port.

Other scripts: `pnpm build` (all), `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm format`.

## Production build

```bash
pnpm --filter @sb/desktop tauri build
```

Produces a native installer. `beforeBuildCommand` rebuilds the overlay first so the latest bundle
is embedded.

## OBS setup

1. Run the app (`pnpm desktop`) and copy the overlay URL from the dashboard
   (default `http://localhost:31337/`).
2. In OBS add a **Browser Source** with that URL, sized to your canvas (1080p / 1440p / 4K all scale).
3. The overlay background is transparent and connects automatically — no extra config.

## Kick connection

Voting reads Kick chat with **zero setup** via Kick's public Pusher WebSocket (anonymous read).
Optional streamer OAuth (for moderator actions) is planned behind the same provider seam — see the
build plan.

## Adding another streaming platform later

Implement the Rust `ChatProvider` trait (and `MediaProvider` if needed) and register a
`ProviderDescriptor` in `packages/shared`. Twitch / YouTube Live / TikTok / Discord slot in there;
Kick is the first implementation.
