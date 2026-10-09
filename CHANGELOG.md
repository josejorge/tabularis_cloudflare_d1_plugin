<!--
File: CHANGELOG.md
Description: Release history of the Cloudflare D1 plugin for Tabularis.
Author: Jose-Jorge HERNANDEZ
Company: Parlee Conseiller, Inc.
Date: 2026-10-09
Last edit date: 2026-10-09
Version: 1.0.0
-->

# Changelog

## 0.1.4 — 2026-10-09

Compatibility pass against Tabularis v0.27.0 (plugin contract reviewed from v0.15.0).

- Unknown RPC methods now return JSON-RPC `-32601` / `Method not found: <name>` instead of `-32603`. The host keys its optional-method fallbacks on that code.
- JSON-RPC notifications (no `id`), such as the host's `cancel`, are no longer answered. Previously they produced a `-32700` line on stdout.
- `.tabularium` now carries `id` (`cloudflare-d1`) and the display `name` (`Cloudflare D1`), plus `sql_dialect: "sqlite"`, `inline_pk`, `auto_increment_keyword`, `serial_type` and `default_username`, so it no longer disagrees with `manifest.json`.
- Release workflow runs `cargo test` and uploads `.tabularium` as a standalone release asset (needed by the registry).
- `Cargo.toml`/`Cargo.lock` version aligned with the manifests (was 0.1.0).
