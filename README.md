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
pnpm dev   # builds the overlay, compiles the Rust backend, and opens the desktop app window
```

`pnpm dev` opens the native **Tauri app window** — that's the actual app. The dashboard needs the Tauri
runtime, so opening `http://localhost:1420` in a plain web browser only shows a "use the desktop app"
notice (`invoke` / event IPC exist only inside the app window). For browser-only frontend iteration use
`pnpm web`. While iterating on the overlay UI, run `pnpm --filter @sb/overlay dev` (port 5174) and point a
browser/OBS there; live data still comes from the desktop app's WebSocket.

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

The **Kick** page offers two providers — pick either, both feed the same vote tally.

### Unofficial (zero setup — recommended)

Enter your channel slug and **Connect**. It reads public chat over Kick's Pusher WebSocket (anonymous
read) — no login, no tunnel, nothing to host. If connecting fails because Kick's channel lookup is
Cloudflare-blocked (a `403`), paste your numeric **chatroom ID** in the optional field (find it at
`https://kick.com/api/v2/channels/<your-slug>` → `chatroom.id`) — only the lookup is blocked, not the
chat stream.

### Official (Kick API — needs a public tunnel)

Uses Kick's official OAuth + webhooks. Chat is delivered only via webhooks, so Kick must reach your PC
through a public HTTPS URL. One-time setup:

1. At **[dev.kick.com](https://dev.kick.com)** create an app → copy the **Client ID** and **Client
   Secret**. Add the redirect URI `http://localhost:31337/oauth/callback`.
2. Expose the app's local receiver with your own stable tunnel, e.g.
   `cloudflared tunnel --url http://localhost:31337` (or an ngrok static domain), and set the app's
   **Webhook URL** at dev.kick.com to `https://<your-tunnel-host>/kick/webhook`.
3. On the Kick page → **Official** → paste Client ID / Secret → **Login with Kick** → approve in the
   browser. The panel should show **authorized** + **subscribed**.

The app does **not** manage the tunnel — run your own. Signature-verified (RSA-SHA256), replay-deduped,
and time-bounded (±5 min). If it shows _authorized_ but _not subscribed_, the webhook URL isn't set at
dev.kick.com yet — set it, then Disconnect → Login again.

> GitHub Pages can't host the webhook (it's static-only and can't reach your PC). If you'd rather avoid a
> tunnel entirely, use the Unofficial provider — it connects outbound and needs no public URL.

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
