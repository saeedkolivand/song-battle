# Song Battle

A desktop app for **Kick.com** streamers to run music tournaments where **chat decides the winners**,
with a transparent **OBS overlay** showing the live matchup.

Build a bracket of songs → start a matchup → viewers vote in Kick chat (type `1`/`2`, `!vote 1`) →
a timer closes voting → the winner auto-advances → the overlay animates it live.

Built with **Tauri v2 · React 19 · TypeScript (strict) · Vite · Tailwind v4 · Zustand · Framer Motion · Rust**.

## Features

- **Battles & brackets** — create a battle (title / description / theme), add songs, shuffle, generate a
  bracket. Three modes: **single elimination**, **double elimination** (winners / losers / grand final +
  bracket reset), and **best of three** (series, first to 2 games).
- **Kick chat voting** — connect a channel and viewers vote on the current matchup with `1`, `2`, `!1`,
  `!2`, `!vote 1`, `!vote 2`. One vote per user, change allowed until the timer ends, spam ignored.
  Moderators can `!reset` votes or `!skip` a matchup from chat.
- **Chat song submissions** — viewers add songs with `!submit <url>` / `!add <url>` while a battle is in
  the lobby (rate-limited, deduped, capped). YouTube / SoundCloud / Spotify links resolve to metadata via
  keyless oEmbed. Toggleable in Settings.
- **Configurable timer** — 10 / 20 / 30 / 60s or custom; auto-closes voting, picks the winner, advances.
- **OBS overlay** — transparent browser source: matchup, artwork, vote bars + percentages + totals,
  countdown (pulses in the last 5s), round badge, series score, winner. Scales 1080p → 4K.
- **OBS WebSocket** — connect to OBS 28+ and **auto-switch scenes** (battle / winner / intermission) off
  the live battle state; set the overlay browser-source URL from the app.
- **Saved tournaments** — keep many; load / delete; JSON export / import; autosave; resume on launch.
- **Hotkeys** — Space (start next), R (reset votes), S (skip), O (open overlay), F (fullscreen overlay).
- **Anonymous mode** — hides submitter identity from the overlay/viewers (stripped server-side).
- **Developer panel** — live event log, vote log, performance metrics, connection status, current-state JSON.
- **Local-first** — SQLite persistence; everything runs on your machine. No account, no cloud.

## Architecture

Three runtimes, one source of truth:

- **Dashboard** — React in the Tauri WebView; talks to Rust over Tauri IPC (`invoke`).
- **Overlay** — React loaded by OBS's embedded Chromium over `http://localhost`. **No Tauri APIs** — a dumb
  projection of server snapshots over a WebSocket.
- **Rust backend** — owns all state (`AppState`). An embedded **axum** HTTP+WebSocket server serves the
  overlay bundle and broadcasts one coalesced snapshot to every overlay client; the dashboard mirrors the
  same snapshot over Tauri events. Kick chat is read in Rust and tallied directly into the source of truth.

```
Kick chat (Rust) ─► tally ─► AppState (truth) ─┬─ Tauri events ──► Dashboard (Zustand cache)
Dashboard ── invoke() ──────────────────────────┘                 axum WS ──► Overlay (OBS)
```

## Repository layout (pnpm + turbo monorepo)

```
apps/desktop    Tauri app: React dashboard (src/) + Rust backend (src-tauri/)
apps/overlay    OBS overlay (separate Vite app; built bundle embedded in the Rust binary)
packages/types  @sb/types  — shared DTOs mirrored from Rust serde
packages/shared @sb/shared — overlay WS client, provider registry, helpers
packages/ui     @sb/ui     — runtime-agnostic presentational components
```

## Prerequisites

- **Node 22+** and **pnpm 11**
- **Rust** stable + the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
  (Windows: WebView2, already present on Win11)

## Development

```bash
pnpm install
pnpm --filter @sb/overlay build   # build the overlay once — it's embedded at Rust compile time
pnpm desktop                      # = pnpm --filter @sb/desktop tauri dev
```

The dashboard shows the overlay URL and lets you create a battle, add songs, connect Kick, and run matchups.
While iterating on the overlay UI, run `pnpm --filter @sb/overlay dev` (port 5174) and point a browser there;
live data still comes from the desktop app's WebSocket.

Other scripts: `pnpm build` (all), `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm format`.

## Production build

```bash
pnpm --filter @sb/desktop tauri build
```

Produces a native installer; `beforeBuildCommand` rebuilds the overlay first so the latest bundle is embedded.

## OBS setup

**Overlay (browser source):**
1. Run the app and copy the overlay URL from the dashboard (default `http://localhost:31337/`).
2. In OBS, add a **Browser Source** with that URL, sized to your canvas (1080p / 1440p / 4K all scale).
3. The background is transparent and it connects automatically — no extra config.

**Auto scene switching (optional):** enable the **OBS WebSocket** server in OBS (Tools → WebSocket Server
Settings, port `4455`), then on the app's **OBS** page connect and map your Battle / Winner / Intermission
scene names. With auto-switch on, the app changes scenes as the battle state changes.

## Kick connection

On the **Kick** page, enter your channel slug and connect. Voting works with **zero setup** — it reads
public chat over Kick's Pusher WebSocket (anonymous read). No login required for voting.

**Chat commands:** vote `1` / `2` / `!1` / `!2` / `!vote 1` / `!vote 2`; submit `!submit <url>` /
`!add <url>` (lobby only); moderators `!reset` / `!skip`.

## Adding another streaming platform

The provider seam is a Rust trait. To add Twitch / YouTube Live / TikTok / Discord: implement
`ChatProvider` (and `MediaProvider` if needed) in `apps/desktop/src-tauri/src/providers/`, and register a
`ProviderDescriptor` in `packages/shared`. Kick is the first implementation.

## Testing

- Rust: `cd apps/desktop/src-tauri && cargo test` (domain logic — brackets, voting, timer, persistence,
  the chat-submission gate, the Kick parser, the WS broadcast) and `cargo clippy`.
- Frontend: `pnpm typecheck` + `pnpm build`.

## Status

The core is complete and reviewed (every change goes through an author → critic loop). Not built /
out of scope: a web "join via code" participant client (dropped), and visual extras like winner particle
effects and drag-and-drop bracket seeding. Live OBS + real-Kick behavior is verified manually.
