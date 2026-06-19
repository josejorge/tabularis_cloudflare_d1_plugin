# Cloudflare D1 Plugin for Tabularis

A [Tabularis](https://tabularis.app) plugin that lets you browse and manage your [Cloudflare D1](https://developers.cloudflare.com/d1/) databases directly from the app.

Built in Rust. Communicates with the Cloudflare D1 REST API.

---

## Features

- Browse all D1 databases in your Cloudflare account
- View tables, columns, indexes, foreign keys, and views
- Run SQL queries with pagination
- Insert, update, and delete rows
- Create and drop tables and indexes
- Create, alter, and drop views
- ER diagram support

---

## Installation

### Option A — Pre-built binary

Download the latest release ZIP for your platform from the [Releases](../../releases) page, extract it, and place the folder in your Tabularis plugins directory:

| OS      | Path |
|---------|------|
| Windows | `%APPDATA%\debba\tabularis\data\plugins\` |
| macOS   | `~/Library/Application Support/debba/tabularis/data/plugins/` |
| Linux   | `~/.local/share/debba/tabularis/data/plugins/` |

> **Note:** The path includes `debba\tabularis\data` — not just `tabularis`. This is how Tabularis resolves its data directory on all platforms.

The result should look like:

```
plugins/
└── cloudflare-d1/
    ├── manifest.json
    └── cloudflare-d1-plugin   (or .exe on Windows)
```

Restart Tabularis and the **Cloudflare D1** driver will appear in the connection picker.

### Option B — Build from source

Prerequisites: [Rust](https://rustup.rs) (stable). On Windows, the build embeds the plugin icon into the `.exe` via `winres`, which requires `rc.exe` from the Windows SDK (already present if you have Visual Studio Build Tools installed).

```bash
git clone https://github.com/josejorge/tabularis_cloudflare_d1_plugin
cd tabularis_cloudflare_d1_plugin
cargo build --release
```

Copy `target/release/cloudflare-d1-plugin[.exe]` and `manifest.json` into a `cloudflare-d1/` folder inside the plugins directory above.

---

## Setup

When creating a new connection in Tabularis, select **Cloudflare D1** and fill in:

| Field        | Value |
|-------------|-------|
| **Host**    | *(leave blank)* |
| **Port**    | *(leave blank)* |
| **Username**| Your Cloudflare **Account ID** |
| **Password**| Your Cloudflare **API Token** |
| **Database**| The name of your D1 database |

### Getting your credentials

**Account ID** — Log in to the [Cloudflare dashboard](https://dash.cloudflare.com), open any domain or the main account page, and copy the Account ID from the right sidebar.

**API Token** — Go to **My Profile → API Tokens → Create Token**. Use the *Edit Cloudflare Workers* template or create a custom token with **D1: Edit** permission.

> You can create one connection per D1 database, all sharing the same Account ID and token.

---

## Limitations

- **No ALTER COLUMN** — SQLite does not support modifying existing columns. Recreate the table instead.
- **No DROP FOREIGN KEY** — Foreign keys are defined at `CREATE TABLE` time and cannot be dropped individually.
- **No schemas** — D1 uses a flat single-schema model.
- **No stored procedures** — D1/SQLite does not support them.

---

## Releasing

Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds the `win-x64` and `linux-x64` binaries and attaches the packaged zips to a GitHub Release automatically.

## Registry

This plugin is listed on [Tabularium](https://registry.tabularis.dev) via the [`.tabularium`](.tabularium) manifest at the repo root. That file is separate from `manifest.json`: `manifest.json` ships inside the release zip and is read by Tabularis at runtime, while `.tabularium` lives in the repo and is read by the registry to list, validate, and surface the plugin.

---

## License

MIT
