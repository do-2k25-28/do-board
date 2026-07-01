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
├── shared/     # Types shared between backend and frontend
└── helm/       # Kubernetes Helm chart
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

## Running

### Docker (recommended)

The only prerequisite is [Docker](https://docs.docker.com/get-docker/) with Compose v2.

#### Development

Hot-reload on every file change: cargo-watch recompiles the backend in a few seconds, dx serve reloads the frontend WASM in the browser.

```sh
# First run - builds the dev images (takes ~10 minutes, cached afterwards)
docker compose -f docker-compose.dev.yml up --build

# Subsequent runs
docker compose -f docker-compose.dev.yml up
```

| Service  | URL                   | Reloads on change to…                     |
|----------|-----------------------|-------------------------------------------|
| Frontend | http://localhost:8080 | `frontend/src/**`, `frontend/input.css`   |
| Backend  | http://localhost:3000 | `backend/src/**`, `shared/src/**`         |

#### Production

Builds optimised release images (Rust binary + nginx serving the WASM bundle).

```sh
docker compose up --build
```

| Service  | URL                   |
|----------|-----------------------|
| Frontend | http://localhost:80   |
| Backend  | http://localhost:3000 |

#### Environment variables

Copy and edit before starting:

```sh
cp .env.example .env
```

| Variable         | Default                      | Description                          |
|------------------|------------------------------|--------------------------------------|
| `ADMIN_EMAIL`    | `admin@example.com`          | Email of the initial admin account   |
| `ADMIN_PASSWORD` | `changeme`                   | Password of the initial admin account |
| `JWT_SECRET`     | `change_me_in_production`    | Secret used to sign JWT tokens - **change this in production** |

The initial admin account is created automatically on first start if it does not already exist.

---

### Kubernetes (Helm)

A Helm chart is provided in [`helm/do-board`](helm/do-board) to deploy do-board on a Kubernetes cluster. It deploys:

- a `backend` Deployment + Service (Axum API)
- a `frontend` Deployment + Service (nginx serving the WASM bundle, reverse-proxying `/api` and `/ws` to the backend)
- an optional `Ingress` exposing the frontend
- a bundled [PostgreSQL](https://github.com/bitnami/charts/tree/main/bitnami/postgresql) instance (can be disabled in favour of an external/managed database)

#### Images

Container images are built and pushed to the GitHub Container Registry by [`.github/workflows/docker.yml`](.github/workflows/docker.yml):

| Trigger                  | Tags pushed                                  |
|---------------------------|-----------------------------------------------|
| Push to `main`            | `latest`, `sha-<short-sha>`                   |
| GitHub Release (`vX.Y.Z`) | `X.Y.Z`, `X.Y`, `X`                            |

Images: `ghcr.io/do-2k25-28/do-board-backend` and `ghcr.io/do-2k25-28/do-board-frontend`. If the packages are private, create an `imagePullSecrets` entry (see below) before installing the chart.

#### Prerequisites

- A Kubernetes cluster and [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) configured to reach it
- [Helm](https://helm.sh/docs/intro/install/) ≥ 3.8
- An ingress controller (e.g. [Traefik](https://doc.traefik.io/traefik/providers/kubernetes-ingress/)) if you want to enable the bundled `Ingress`

#### Install

```sh
cd helm/do-board

# Fetch the bundled PostgreSQL chart dependency
helm dependency update

helm install do-board . \
  --namespace do-board --create-namespace \
  --set env.jwtSecret="$(openssl rand -hex 32)" \
  --set env.adminEmail=admin@example.com \
  --set env.adminPassword="$(openssl rand -hex 16)" \
  --set ingress.enabled=true \
  --set ingress.className=traefik \
  --set ingress.host=do-board.example.com
```

Without `ingress.enabled`, reach the app with:

```sh
kubectl port-forward -n do-board svc/do-board-frontend 8080:80
```

#### Key values

| Value                       | Default                                       | Description                                              |
|------------------------------|------------------------------------------------|------------------------------------------------------------|
| `backend.image.repository`   | `ghcr.io/do-2k25-28/do-board-backend`         | Backend image                                             |
| `frontend.image.repository`  | `ghcr.io/do-2k25-28/do-board-frontend`        | Frontend image                                             |
| `backend.image.tag` / `frontend.image.tag` | `""` (→ `.Chart.AppVersion`, i.e. `latest`) | Override to pin a release tag, e.g. `1.2.0` or `sha-abcdef0` |
| `imagePullSecrets`           | `[]`                                            | Names of secrets to pull private GHCR images               |
| `env.jwtSecret` / `env.adminEmail` / `env.adminPassword` | see `values.yaml` | App credentials - **override in production**, or set `existingSecret` to a Secret you manage yourself |
| `config.gtfsStaticUrl` / `config.gtfsRtUrl` | Montpellier TaM GTFS feeds | Public transport widget data source |
| `postgresql.enabled`         | `true`                                          | Set to `false` to use an external database via `externalDatabase.url` |
| `ingress.enabled`             | `false`                                         | Expose the frontend through an Ingress                    |

See [`helm/do-board/values.yaml`](helm/do-board/values.yaml) for the full list, and run `helm show values helm/do-board` or `helm template helm/do-board` to inspect the rendered manifests before installing.

#### Upgrade / uninstall

```sh
helm upgrade do-board helm/do-board -n do-board -f my-values.yaml
helm uninstall do-board -n do-board
```

---

### Manual (without Docker)

#### Requirements

- [Rust](https://rustup.rs/) ≥ 1.80
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started) - `cargo install dioxus-cli`
- [Tailwind CSS CLI](https://github.com/tailwindlabs/tailwindcss/releases/latest) - standalone binary, no Node.js required
- `wasm32-unknown-unknown` target
- A running PostgreSQL instance

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli

# Tailwind CSS standalone binary (Linux x86_64)
curl -sL https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 \
  -o ~/.local/bin/tailwindcss && chmod +x ~/.local/bin/tailwindcss
```

#### Backend

```sh
export DATABASE_URL=postgresql://user:password@localhost:5432/doboard
export ADMIN_EMAIL=admin@example.com
export ADMIN_PASSWORD=changeme
export JWT_SECRET=my_secret

cargo run -p backend
# API available at http://localhost:3000
```

#### Frontend

Two terminals running in parallel:

```sh
# Terminal 1 - Tailwind CSS (watch mode)
cd frontend
tailwindcss -i input.css -o assets/tailwind.css --watch

# Terminal 2 - Dioxus dev server
cd frontend
API_BASE=http://localhost:3000 dx serve
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

MIT - see [LICENSE](LICENSE)
