<!--
File: CHANGELOG.md
Description: Release history of the Cloudflare D1 plugin for Tabularis.
Author: Jose-Jorge HERNANDEZ
Company: Parlee Conseiller, Inc.
Date: 2026-10-09
Last edit date: 2026-10-10
Version: 2.1.0
-->

# Changelog

## 1.0.1 — 2026-10-10

- Plugin setting **Default Account ID**: when a connection leaves Username blank, this Account ID is used (received via `initialize`). The API token deliberately stays in the connection's Password field, not in plugin settings.
- Connection-form help box translated (English, Spanish) via `locales/<lang>.json`; English is the fallback. Release zips now include `locales/`.
- Account ID error message now mentions the plugin setting.

## 1.0.0 — 2026-10-09

First stable release.

- Visual EXPLAIN: `explain_query` runs `EXPLAIN QUERY PLAN` and returns a parsed plan tree (works on any host that supports plugin EXPLAIN; `explain` capability enabled). SQLite has no cost model or ANALYZE, so those fields are empty.
- `PRAGMA` and `EXPLAIN` statements now return their rows in the grid instead of an empty result (they are never wrapped for pagination).
- Triggers: `get_triggers`, `get_trigger_definition`, `create_trigger`, `drop_trigger` (from `sqlite_master`); `triggers` capability enabled. Timing/event are parsed from the stored `CREATE TRIGGER` SQL.
- Table/column comments intentionally not implemented: D1 has no comment metadata.
- Live behaviour against a real D1 database was not exercised in CI (unit tests cover parsing and plan building); EXPLAIN/triggers should be tried once in Tabularis.

## 0.1.7 — 2026-10-09

- Added `linux-arm64` release build (native `ubuntu-24.04-arm` runner), requested in issue #3.

## 0.1.6 — 2026-10-09

- Connection form now shows a help box (UI extension `ui/connection-help.js`, slot `connection-modal.extra_fields`) explaining that Username = Cloudflare Account ID, Password = API token ("D1 Edit"), Host/Port unused, with a link to create a token. Requires a Tabularis build with plugin UI extensions; older hosts simply don't show it.
- Release zips now include the `ui/` folder.

## 0.1.5 — 2026-10-09

- macOS builds: release workflow now also builds `darwin-arm64` (Apple Silicon) and `darwin-x64` (Intel) and `registry.json` lists them. Binaries are unsigned and untested on real hardware (CI builds and unit-tests only).

## 0.1.4 — 2026-10-09

Compatibility pass against Tabularis v0.27.0 (plugin contract reviewed from v0.15.0).

- Unknown RPC methods now return JSON-RPC `-32601` / `Method not found: <name>` instead of `-32603`. The host keys its optional-method fallbacks on that code.
- JSON-RPC notifications (no `id`), such as the host's `cancel`, are no longer answered. Previously they produced a `-32700` line on stdout.
- `.tabularium` now carries `id` (`cloudflare-d1`) and the display `name` (`Cloudflare D1`), plus `sql_dialect: "sqlite"`, `inline_pk`, `auto_increment_keyword`, `serial_type` and `default_username`, so it no longer disagrees with `manifest.json`.
- Release workflow runs `cargo test` and uploads `.tabularium` as a standalone release asset (needed by the registry).
- `Cargo.toml`/`Cargo.lock` version aligned with the manifests (was 0.1.0).
