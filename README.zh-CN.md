# 龙鉴 (Loong Sentinel)

<p align="center">
  <strong>AI 驱动的多渠道舆情监测系统</strong>
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

龙鉴是基于 [Loong](https://github.com/eastreams/loong) 智能体底座构建的轻量级多渠道舆情监测系统。通过 AI 驱动的情感分析、事件聚类与知识图谱投影，实现从信息采集、实体归因、关系追踪到预警推送的全链路自动化。

## 核心能力

- **多渠道采集** — Telegram、飞书/Lark、Discord（通过 Loong 底座可扩展至 20+ 渠道）
- **AI 情感分析** — 正/负/中性三分类 + 立场检测，准确率 >85%，单条延迟 <2s
- **实体与关系抽取** — 命名实体、别名、关系、证据片段、时间表达
- **事件聚类与趋势** — 语义嵌入 + 增量聚类，支持 1h/6h/24h 时间窗口趋势
- **分级预警** — 关注/警告/紧急三级，触发后 <60s 送达指定渠道
- **知识图谱** — 实体-事件-来源关系图，支持可解释查询路径
- **调查工作台** — 混合检索：PostgreSQL 筛选 + 向量召回 + Neo4j 上下文扩展

## 架构总览

```text
┌──────────────────────────────────────────────────────────────────┐
│                    Web 前端 (React + TypeScript)                  │
│  登录鉴权 │ 仪表盘 │ 预警管理 │ 图谱可视化 │ 趋势分析 │ 调查台  │
├──────────────────────────────────────────────────────────────────┤
│                    业务逻辑层                                     │
│  去重清洗 │ 情感分析 │ 实体/关系抽取 │ 事件聚类 │ 预警引擎        │
├──────────────────────────────────────────────────────────────────┤
│                    数据平面                                       │
│  PostgreSQL FactStore │ Neo4j GraphStore │ 向量/HNSW             │
├──────────────────────────────────────────────────────────────────┤
│                    Loong 智能体底座                               │
│  Channel(20+) │ Provider(40+) │ Spec │ Memory │ Kernel │ Gateway │
└──────────────────────────────────────────────────────────────────┘
```

### 核心设计原则

- PostgreSQL 是舆情业务唯一事实源
- Neo4j 是异步投影的关系读模型——投影失败不阻塞事实入库与预警
- 禁止 PostgreSQL 与 Neo4j 同步双写，所有图谱变化从 outbox 重放
- `tenant_id` 贯穿 FactStore、GraphStore、outbox、checkpoint 与 API 边界
- 向量索引回答"语义上像不像"，图谱回答"关系上怎么连"，PostgreSQL 回答"事实状态是什么"

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust (Edition 2024)，严格 Clippy，`#![forbid(unsafe_code)]` |
| 事实存储 | PostgreSQL 16+ |
| 图谱存储 | Neo4j 5.x (neo4rs 驱动) |
| HTTP 框架 | Axum 0.8 |
| 前端 | React 18 + TypeScript, Vite |
| UI 组件库 | Ant Design 5.x |
| 状态管理 | Zustand |
| 图谱可视化 | Cytoscape.js |
| 图表 | ECharts |
| 向量检索 | pgvector + HNSW |
| 编排引擎 | Loong Spec（重试、熔断、自适应并发） |
| 容器化 | Docker + docker-compose |

## 快速开始

### 前置条件

- Rust 工具链 (edition 2024)
- PostgreSQL 16+
- Neo4j 5.x（Phase 2+ 需要）
- Docker + docker-compose（推荐）

### 开发环境搭建

```bash
# 克隆仓库
git clone https://github.com/xuanli520/loongside.git
cd loongside

# 启动基础设施
docker-compose up -d postgres neo4j

# 构建 sentinel pack
cargo build --workspace

# 运行数据库迁移
cargo run --bin daemon -- migrate

# 启动服务
cargo run --bin daemon -- sentinel serve
```

### 前端

```bash
cd frontend
npm install
npm run dev
```

## 数据流

```text
[社媒 / 新闻 / 论坛]
        │
        ▼
   源适配采集 ──── Channel.serve / Spec cron
        │
        ▼
   去重 / 清洗 ──── URL 指纹 + SimHash + 正文提取
        │
        ├──────────────┬──────────────────────────┐
        ▼              ▼                          ▼
  PostgreSQL      情感/立场分析              实体/关系抽取
  FactStore            │                          │
        │              └──────────┬──────────────┘
        │                         ▼
        │                    事件聚类 / 消歧
        │                         │
        │                         ▼
        │                    热度评估 / 预警引擎
        │                         │
        │                         ├──────► Channel.send（预警推送）
        │                         └──────► Gateway API
        │
        └── graph_outbox ────────────────► 投影 Worker
                                                │
                                                ▼
                                           Neo4j GraphStore
```

## API 接口

| 接口 | 用途 |
|------|------|
| `GET /events/{id}/graph?depth=2&window=24h` | 事件子图：实体、来源、文档、相邻事件 |
| `GET /events/{id}/explain` | 预警解释链：alert → event → entity → mention → document → source |
| `GET /entities/{id}/related-events?window=24h` | 实体关联事件、情感倾向、预警次数 |
| `GET /sources/{id}/propagation?event_id=...` | 事件传播来源与扩散时间线 |
| `POST /investigations/search` | 混合检索：PG 候选 + 向量召回 + Neo4j 扩展 |
| `GET /projection/status` | outbox 积压、投影延迟、checkpoint、重试状态 |

## 路线图

| 阶段 | 重点 | 状态 |
|------|------|------|
| Phase 0 | FactStore 骨架 + 前端初始化 | 进行中 |
| Phase 1 | MVP — 采集 + 分析 + 预警 | 计划中 |
| Phase 2 | 图谱投影 + 图查询 + 趋势 | 计划中 |
| Phase 2.5 | memory-postgres 迁移（b 版） | 计划中 |
| Phase 3 | 加固 + 扩展 | 计划中 |

## 项目结构

```text
loongside/
├── crates/
│   ├── contracts/     # 稳定契约词汇表
│   ├── kernel/        # 策略引擎、审计、执行平面
│   ├── protocol/      # 传输基础
│   ├── app/           # Provider、频道、工具、sentinel pack
│   ├── spec/          # 编排引擎、WASM 运行时
│   ├── bench/         # 基准测试
│   └── daemon/        # CLI 二进制 + gateway 服务
├── frontend/          # React + TypeScript Web 应用
├── docs/              # 设计文档、蓝图、项目计划
├── scripts/           # 构建与部署脚本
└── tests/             # 集成测试
```

## 文档

- [系统蓝图](../docs/BLUEPRINT.md) — 完整系统蓝图与 schema 设计
- [项目计划](../docs/PROJECT_PLAN.md) — 4 周冲刺计划与团队分工
- [决策指南](../docs/DECISION_GUIDE.md) — 架构决策与实施指南
- [架构分析](../docs/ARCHITECTURE_ANALYSIS.md) — Loong 底座复用分析

## 贡献

欢迎贡献。工作流与规范见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT
