# Loong Sentinel (龙鉴)

<p align="center">
  <strong>AI-Driven Multi-Channel Sentiment Monitoring System</strong>
</p>

<p align="center">
  <a href="https://github.com/xuanli520/loongside"><img src="https://img.shields.io/github/stars/xuanli520/loongside?style=flat-square" alt="Stars" /></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License: MIT" /></a>
  <img src="https://img.shields.io/badge/rust-edition%202024-orange.svg?style=flat-square" alt="Rust Edition 2024" />
  <img src="https://img.shields.io/badge/status-alpha-yellow.svg?style=flat-square" alt="Status: Alpha" />
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

---

Loong Sentinel is a lightweight multi-channel sentiment monitoring system built on the [Loong](https://github.com/eastreams/loong) agent base. It automates the full pipeline from information collection, entity attribution, relationship tracking to alert delivery — powered by AI-driven sentiment analysis, event clustering, and knowledge graph projection.

## Features

- **Multi-channel collection** — Telegram, Feishu/Lark, Discord (extensible to 20+ channels via Loong base)
- **AI sentiment analysis** — Positive/negative/neutral classification with stance detection, >85% accuracy, <2s latency per item
- **Entity & relationship extraction** — Named entities, aliases, relationships, evidence spans, temporal expressions
- **Event clustering & trending** — Semantic embedding + incremental clustering with 1h/6h/24h trend windows
- **Tiered alerting** — Watch/Warning/Critical levels, <60s delivery to designated channels
- **Knowledge graph** — Entity-event-source relationship graph with explainable query paths
- **Investigation workspace** — Hybrid retrieval: PostgreSQL filtering + vector recall + Neo4j context expansion

## Architecture

```text
┌──────────────────────────────────────────────────────────────────┐
│                    Web Frontend (React + TypeScript)              │
│  Auth │ Dashboard │ Alerts │ Graph Viz │ Trends │ Investigation  │
├──────────────────────────────────────────────────────────────────┤
│                    Business Logic Layer                           │
│  Dedup/Clean │ Sentiment │ Entity/Relation │ Clustering │ Alert  │
├──────────────────────────────────────────────────────────────────┤
│                    Data Plane                                     │
│  PostgreSQL FactStore │ Neo4j GraphStore │ Vector/HNSW           │
├──────────────────────────────────────────────────────────────────┤
│                    Loong Agent Base                               │
│  Channel(20+) │ Provider(40+) │ Spec │ Memory │ Kernel │ Gateway │
└──────────────────────────────────────────────────────────────────┘
```

### Core Design Principles

- PostgreSQL is the single source of truth for all sentiment business facts
- Neo4j is an async-projected read model for relationship queries — projection failures never block fact ingestion or alerting
- No synchronous dual-write between PostgreSQL and Neo4j; all graph changes replay from outbox
- `tenant_id` permeates FactStore, GraphStore, outbox, checkpoint, and API boundaries
- Vector index answers "semantically similar?"; graph answers "how are they connected?"; PostgreSQL answers "what is the factual state?"

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust (Edition 2024), strict Clippy, `#![forbid(unsafe_code)]` |
| Fact storage | PostgreSQL 16+ |
| Graph storage | Neo4j 5.x (neo4rs driver) |
| HTTP framework | Axum 0.8 |
| Frontend | React 18 + TypeScript, Vite |
| UI components | Ant Design 5.x |
| State management | Zustand |
| Graph visualization | Cytoscape.js |
| Charts | ECharts |
| Vector search | pgvector + HNSW |
| Orchestration | Loong Spec engine (retry, circuit-breaker, adaptive concurrency) |
| Containerization | Docker + docker-compose |

## Quick Start

### Prerequisites

- Rust toolchain (edition 2024)
- PostgreSQL 16+
- Neo4j 5.x (for Phase 2+)
- Docker + docker-compose (recommended)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/xuanli520/loongside.git
cd loongside

# Start infrastructure
docker-compose up -d postgres neo4j

# Build the sentinel pack
cargo build --workspace

# Run database migrations
cargo run --bin daemon -- migrate

# Start the service
cargo run --bin daemon -- sentinel serve
```

### Frontend

```bash
cd frontend
npm install
npm run dev
```

## Data Flow

```text
[Social Media / News / Forums]
        │
        ▼
   Source Collection ──── Channel.serve / Spec cron
        │
        ▼
   Dedup / Clean ──────── URL fingerprint + SimHash + content extraction
        │
        ├──────────────┬──────────────────────────┐
        ▼              ▼                          ▼
  PostgreSQL      Sentiment/Stance         Entity/Relation
  FactStore       Analysis                 Extraction
        │              │                          │
        │              └──────────┬──────────────┘
        │                         ▼
        │                    Event Clustering / Disambiguation
        │                         │
        │                         ▼
        │                    Heat Scoring / Alert Engine
        │                         │
        │                         ├──────► Channel.send (alerts)
        │                         └──────► Gateway API
        │
        └── graph_outbox ────────────────► Projection Worker
                                                │
                                                ▼
                                           Neo4j GraphStore
```

## API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /events/{id}/graph?depth=2&window=24h` | Event subgraph: entities, sources, documents, adjacent events |
| `GET /events/{id}/explain` | Alert explanation chain: alert → event → entity → mention → document → source |
| `GET /entities/{id}/related-events?window=24h` | Entity-related events, sentiment, alert count |
| `GET /sources/{id}/propagation?event_id=...` | Event propagation sources and spread timeline |
| `POST /investigations/search` | Hybrid retrieval: PG candidates + vector recall + Neo4j expansion |
| `GET /projection/status` | Outbox backlog, projection lag, checkpoint, retry status |

## Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| Phase 0 | FactStore skeleton + frontend init | In Progress |
| Phase 1 | MVP — collection + analysis + alerting | Planned |
| Phase 2 | Graph projection + graph queries + trends | Planned |
| Phase 2.5 | memory-postgres migration (b-release) | Planned |
| Phase 3 | Hardening + scaling | Planned |

## Project Structure

```text
loongside/
├── crates/
│   ├── contracts/     # Stable contract vocabulary
│   ├── kernel/        # Policy engine, audit, execution planes
│   ├── protocol/      # Transport foundation
│   ├── app/           # Providers, channels, tools, sentinel pack
│   ├── spec/          # Orchestration engine, WASM runtime
│   ├── bench/         # Benchmarks
│   └── daemon/        # CLI binary + gateway service
├── frontend/          # React + TypeScript web application
├── docs/              # Design docs, blueprints, project plan
├── scripts/           # Build and deployment scripts
└── tests/             # Integration tests
```

## Documentation

- [Blueprint](../docs/BLUEPRINT.md) — Full system blueprint and schema design
- [Project Plan](../docs/PROJECT_PLAN.md) — 4-week sprint plan with team assignments
- [Decision Guide](../docs/DECISION_GUIDE.md) — Architecture decisions and implementation guide
- [Architecture Analysis](../docs/ARCHITECTURE_ANALYSIS.md) — Loong base reuse analysis

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for workflow and conventions.

## License

MIT
