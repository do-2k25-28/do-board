# do-board

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Dioxus](https://img.shields.io/badge/Dioxus-0.7-blue?style=flat&logo=rust&logoColor=white)](https://dioxuslabs.com)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=webassembly&logoColor=white)](https://webassembly.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat)](LICENSE)
[![Issues](https://img.shields.io/github/issues/do-2k25-28/do-board?style=flat)](https://github.com/do-2k25-28/do-board/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat)](https://github.com/do-2k25-28/do-board/pulls)
[![GitHub Stars](https://img.shields.io/github/stars/do-2k25-28/do-board?style=flat)](https://github.com/do-2k25-28/do-board/stargazers)

Open-source customizable dashboard for TV displays, written in Rust.

## Overview

do-board is a web application designed to run on a television screen. It displays configurable widgets in real time: public transport schedules, promotion birthdays, embedded dashboards (Grafana, etc.), weather, planning, and more.

## Architecture

Rust workspace with two crates and a shared types library:

```
do-board/
├── backend/    # REST API (Axum, port 3000)
├── frontend/   # SPA compiled to WebAssembly (Dioxus)
└── shared/     # Types shared between backend and frontend
```

| Crate      | Role                              | Technology     |
|------------|-----------------------------------|----------------|
| `backend`  | HTTP API, data aggregation        | Axum, Tokio    |
| `frontend` | Browser UI, widget rendering      | Dioxus, WASM   |
| `shared`   | Common data types (Dashboard, Widget, …) | Serde  |

## Widgets

| Type        | Description                          |
|-------------|--------------------------------------|
| `weather`   | Current weather conditions           |
| `transport` | Public transport schedules           |
| `birthdays` | Promotion birthdays                  |
| `iframe`    | Embedded external URL (Grafana, …)   |
| `clock`     | Current time and date                |
| `planning`  | Weekly schedule                      |

## Requirements

- [Rust](https://rustup.rs/) ≥ 1.80
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started) — `cargo install dioxus-cli`
- [Tailwind CSS CLI](https://github.com/tailwindlabs/tailwindcss/releases/latest) — standalone binary, no Node.js required
- `wasm32-unknown-unknown` target

```sh
# Rust WASM target
rustup target add wasm32-unknown-unknown

# Dioxus CLI
cargo install dioxus-cli

# Tailwind CSS standalone binary (Linux x86_64)
curl -sL https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 \
  -o ~/.local/bin/tailwindcss && chmod +x ~/.local/bin/tailwindcss
```

## Running

### Backend

```sh
cargo run -p backend
# API available at http://localhost:3000
```

### Frontend

The frontend requires two processes running in parallel.

**Terminal 1 — Tailwind CSS (watch mode)**

```sh
cd frontend
tailwindcss -i input.css -o assets/tailwind.css --watch
```

**Terminal 2 — Dioxus dev server**

```sh
cd frontend
dx serve
# UI available at http://localhost:8080
```

## Star History

<div align="center">
  <a href="https://www.star-history.com/?type=date&repos=do-2k25-28%2Fdo-board">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=do-2k25-28/do-board&type=date&theme=dark&legend=top-left" />
      <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=do-2k25-28/do-board&type=date&legend=top-left" />
      <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=do-2k25-28/do-board&type=date&legend=top-left" />
    </picture>
  </a>
</div>

## License

MIT — see [LICENSE](LICENSE)
