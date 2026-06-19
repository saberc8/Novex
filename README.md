# Novex

Novex 是一套面向企业交付的 AI Agent 基座。它不是单点 AI 应用，而是把账号、租户、权限、知识库、模型路由、Agent 运行时、工具、MCP、连接器、记忆、评测、模板和交付流程沉淀成可复用平台能力，再按客户、行业和场景组合成具体应用。

当前仓库已经具备 Rust + Next.js 的 RBAC 控制平面、AI Foundation Rust crates、前台应用模板、parser worker、POC Docker 运行环境和交付模板。完整架构长文见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，本 README 只保留项目首页需要的入口信息。

## 产品截图

<table>
  <tr>
    <td width="100%">
      <img src="images/poc-codexlike.png" alt="Codex-like Agent 工作台" width="100%" />
      <br />
      <sub>Codex-like Agent 工作台：联网搜索、模型输出和运行事件。</sub>
    </td>
  </tr>
  <tr>
    <td width="100%">
      <img src="images/poc-notebooklm.png" alt="NotebookLM-like 知识工作区" width="100%" />
      <br />
      <sub>NotebookLM-like 知识工作区：资料、对话和内容生成。</sub>
    </td>
  </tr>
  <tr>
    <td width="100%">
      <img src="images/rbac-admin.png" alt="AI 基座模型管理" width="100%" />
      <br />
      <sub>AI 基座模型管理：模型路由、密钥占位和健康检查。</sub>
    </td>
  </tr>
  <tr>
    <td width="100%">
      <img src="images/skills-admin.png" alt="AI Skills 导入与管理" width="100%" />
      <br />
      <sub>AI Skills 导入与管理：GitHub Skill 解析、预览和安装。</sub>
    </td>
  </tr>
</table>

## 能力范围

- 控制平面：租户、用户、角色、菜单、部门、文件、配置、调度、审计、在线用户和系统日志。
- AI 基座：模型注册与路由、RAG、Agent Runtime、Run Graph、工具执行、MCP 网关、连接器、插件、触发器、记忆、评测和 trace。
- 客户应用：管理后台、知识库问答、员工培训、Agent 工作台、客服 Agent 和 Codex-like POC 应用。
- 交付体系：行业模板、客户模板、默认角色/菜单/技能/评测集、环境配置和交付文档。
- 运行支撑：PostgreSQL、Milvus、Redis、RabbitMQ、MinIO、Neo4j、Parser Worker 和可选模型运行时。

## 快速启动

POC 默认复用外部 `docker-common` 基础设施。先启动共享基础服务，再启动 Novex 项目服务。

```bash
cd /Users/yusenlin/Avalon/freedom/2026/aimanju/aether-loom
docker compose up -d postgres redis rabbitmq etcd minio milvus attu neo4j

cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
./scripts/run-poc.sh
```

`scripts/run-poc.sh` 会读取 `infra/.env.poc`；如果该文件不存在，会从 `infra/.env.poc.example` 复制生成。脚本还会检查共享容器、创建缺失的 `novex` 数据库、校验 AI 相关环境变量，并启动 backend、parser-worker 和 POC 前端应用。

常用命令：

```bash
./scripts/run-poc.sh env       # 检查 LLM / Embedding / Reranker / Parser 等配置
./scripts/run-poc.sh status    # 查看服务状态
./scripts/run-poc.sh logs      # 跟踪日志
./scripts/run-poc.sh down      # 停止 POC 服务
./scripts/run-poc.sh pull      # 在可访问镜像源时拉取缺失镜像
```

默认访问地址：

| 服务 | 地址 |
| --- | --- |
| Backend | `http://localhost:4398` |
| Admin | `http://localhost:4399` |
| Training Web | `http://localhost:4401` |
| Chat Web | `http://localhost:4402` |
| Agent Workspace | `http://localhost:4403` |
| RabbitMQ UI | `http://localhost:15673` |
| MinIO Console | `http://localhost:19011` |
| Attu | `http://localhost:18000` |
| Neo4j Browser | `http://localhost:17474` |

健康检查：

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
```

更多基础设施说明见 [infra/README.md](infra/README.md)。

## 环境配置

本地 POC 的唯一环境入口是 `infra/.env.poc`，提交到仓库的是 schema/defaults 文件 `infra/.env.poc.example`。不要把真实密钥写入 example 文件。

主要配置组：

- 基础运行：`AUTH_JWT_SECRET`、`BACKEND_PORT`、`ADMIN_PORT`、`CHAT_WEB_PORT`、`TRAINING_WEB_PORT`、`AGENT_WORKSPACE_PORT`
- 共享基础设施：`COMMON_DOCKER_NETWORK`、`DATABASE_URL`、`REDIS_URL`、`RABBITMQ_URL`、`MILVUS_ENDPOINT`、`MINIO_ENDPOINT`
- 模型能力：`LLM_API_KEY`、`LLM_BASE_URL`、`LLM_MODEL`
- 向量与重排：`EMBEDDING_API_KEY`、`EMBEDDING_BASE_URL`、`EMBEDDING_MODEL`、`RERANKER_API_KEY`、`RERANKER_BASE_URL`、`RERANKER_MODEL`
- Parser：`PARSER_CALLBACK_TOKEN`、`PARSER_WORKER_MODE`、`MINERU_TOKEN`、`MINERU_TIMEOUT_SECONDS`
- 外部连接器：`GITHUB_CONNECTOR_TOKEN`、`GITHUB_OAUTH_CLIENT_ID`、`GITHUB_OAUTH_CLIENT_SECRET`、`FEISHU_WEBHOOK_URL`
- 媒体工具：`RIGHT_CODE_DRAW_BASE_URL`、`RIGHT_CODE_DRAW_API_KEY`

如果缺少部分外部 AI 配置，平台仍可启动；对应的 live chat、RAG embedding、rerank、PDF/Office/Image 解析或媒体工具能力会降级或不可用。

## 本地开发

后端是 Cargo workspace：

```bash
cargo run -p backend-rust
cargo test --workspace
```

单独运行后端时，使用 `backend/.env.example` 作为本地 `.env` 模板，并确保 PostgreSQL、Redis、RabbitMQ、Milvus 等依赖地址与实际运行方式一致。

前端应用分别维护 `package.json`，使用 pnpm：

```bash
cd admin && pnpm install && pnpm dev
cd apps/training-web && pnpm install && pnpm dev
cd apps/chat-web && pnpm install && pnpm dev
cd apps/agent-workspace && pnpm install && pnpm dev
```

常用前端检查：

```bash
pnpm typecheck
pnpm test
pnpm lint
pnpm build
```

在对应前端目录内执行这些命令。

## 仓库结构

```text
Novex/
  backend/                 Rust Axum API，RBAC、控制平面、业务编排和 HTTP 接口
  crates/                  AI Foundation Rust crates
    novex-ai-core/         通用领域模型、Run Graph、Trace、Policy
    novex-model/           模型注册、路由、能力描述、健康检查、用量
    novex-provider-client/ 模型 provider HTTP/stream/media transport
    novex-rag/             chunk、embedding、检索、rerank、context builder
    novex-agent*/          Agent 协议、运行时、planner、tool loop
    novex-tools/           tool registry、tool executor、权限策略
    novex-connectors/      GitHub、飞书、网页、数据库等连接器
    novex-mcp/             MCP gateway、server/tool discovery、授权
    novex-plugin/          插件 manifest、安装、版本和能力声明
    novex-skill/           技能包定义、导入和策略
    novex-trigger/         webhook、schedule、plugin event、外部事件路由
    novex-memory/          session/user/org/project memory
    novex-eval/            eval runner、指标、报告
    novex-trace/           trace bundle、replay、eval capture boundary
  admin/                   Next.js 管理后台
  apps/
    training-web/          员工培训模板
    chat-web/              默认 LLM Chat / 知识库问答前台
    agent-workspace/       Agent 工作台模板
    codex-app-poc/         Codex-like POC 应用
    customer-service-agent/客服 Agent 模板应用
  services/
    parser-worker/         Python sidecar，文档解析、MinerU、OCR、格式转换
    model-runtime/         可选模型运行时 adapter
  templates/               客户交付模板、默认菜单、技能、评测集和 smoke 脚本
  infra/                   Docker Compose、POC env、基础设施说明
  docs/                    架构、计划和交付文档
  scripts/                 POC 启动和 smoke 脚本
```

## 架构边界

Novex 采用 Rust first、Python sidecar、Next.js frontend：

- Rust 负责长期稳定、强权限、强并发、强审计的核心控制面和 AI 编排能力。
- Python 只作为插件型 sidecar，承载 MinerU、LibreOffice、OCR、文档版面分析、本地模型 adapter 或实验性 connector。
- Next.js 负责管理后台和客户可交付前台模板。
- 跨语言调用通过 HTTP、queue job、MCP/tool schema 或稳定 API 完成；sidecar 不直接绕过后端访问核心业务表。

总体分层：

```text
Customer Apps
  培训系统 / 知识库问答 / 客服辅助 / 研发助手 / 运营自动化
        |
App Template Layer
  标准前台模板 / 客户品牌 / 行业页面 / 业务工作台 / 管理后台
        |
AI Foundation Layer
  Agent Runtime / Run Graph / RAG / Model / Tools / MCP / Eval / Trace
        |
Control Plane
  RBAC / Tenant / Audit / Config / Scheduler / File / Observability
        |
Infrastructure
  PostgreSQL / Milvus / Redis / RabbitMQ / MinIO / Parser Worker / Model Runtime
```

设计原则：

1. 权限优先：知识库、工具、技能、记忆、会话和评测数据都必须经过租户、用户、角色和资源权限过滤。
2. 基座复用：客户差异优先沉淀为配置、模板、技能包、连接器、页面和运行策略。
3. RAG 与 Agent 分离：知识问答走 RAG，源码检索、工具执行和任务自动化走 Agentic Search + Tool Use。
4. 资源可控：POC 阶段优先复用 PostgreSQL、Milvus Standalone、外部模型 API、OpenAI-compatible endpoint 和独立 parser worker。
5. 可观测、可评测、可回放：检索、重排、模型调用、工具调用、意图路由和 ReAct 步骤都要留下 trace。
6. 模型可替换：LLM、Embedding、Rerank、VLM 等通过统一模型适配层接入，支持公有云、内网 endpoint、本地模型和租户级路由。

## 文档索引

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：完整 AI Agent Foundation 架构说明。
- [infra/README.md](infra/README.md)：共享 Docker 基础设施、默认端口和 POC 运行说明。
- [docs/delivery/novex-customer-delivery.md](docs/delivery/novex-customer-delivery.md)：客户交付边界和交付包说明。
- [docs/plans](docs/plans)：按日期沉淀的设计和实施计划。
- [templates/README.md](templates/README.md)：客户模板和 smoke 脚本入口。

## 维护约定

- 根 README 保持入口级别，不承载完整架构长文；深入设计写入 `docs/`。
- 新增运行依赖时，同步更新 `infra/.env.poc.example`、`infra/README.md` 和本 README 的环境配置摘要。
- 新增前台应用时，同步更新 `apps/` 目录说明、默认端口和 POC 启动脚本。
- 新增客户模板时，同步更新 `templates/` 下的 README、`template.json` 和 smoke 脚本。
