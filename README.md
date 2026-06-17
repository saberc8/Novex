# Novex AI Agent Foundation Architecture

## 1. 定位

Novex 的目标不是再做一个单点 AI 应用，而是做一套可复用的 AI Agent 基座。它面向的交付方式是：底层能力统一沉淀，上层按客户、行业、场景快速组合。例如客户需要一个 AI 员工培训系统、企业知识库问答、客服辅助、研发助手、运营自动化系统时，都复用同一套权限、知识库、工具、技能、记忆、评测和交付体系。

当前 Novex 已经有一个 Rust + Next.js 的 RBAC 启动模板：

- `backend`: Rust / Axum / SQLx / PostgreSQL，已有认证、用户、角色、菜单、部门、文件、配置、调度、日志、在线用户等模块。
- `admin`: Next.js / React / Tailwind / shadcn 风格组件，已有登录、后台布局、系统管理、权限控制和基础测试。
- `migrations`: 已有系统表、种子数据、权限和调度表。

AI 基座应当接在这套 RBAC 模板之上，而不是绕开它重做账号体系。Novex 负责控制平面、权限平面、业务编排平面和交付平面；Dify、FastGPT、Codex、Hive 的设计可以作为参考或局部适配对象，但不建议让它们成为不可替换的核心依赖。

## 2. 设计原则

1. 权限优先：所有知识库、工具、技能、记忆、会话、评测数据都必须经过租户、用户、角色和资源权限过滤。
2. 基座复用：客户差异尽量沉淀为配置、模板、技能包、运行编排策略、连接器和前台页面，而不是 fork 一套代码。
3. RAG 和 Agent 分离：知识库问答走 RAG；源码、工具执行、任务自动化走 Agentic Search 和 Tool Use，不把所有问题都抽象成向量召回。
4. 资源可控：POC 阶段优先使用 PostgreSQL + Milvus Standalone、外部模型 API 或一个 OpenAI-compatible 内网模型端点、独立 parser worker，避免一次性引入 Dify 全家桶、K8s、GPU 服务。
5. 可观测和可评测：每一次检索、重排、模型调用、工具调用、意图路由、ReAct 步骤都必须可追踪、可回放、可评测。
6. 可交付：每个客户项目必须能从标准模板生成独立配置，包括菜单、角色、知识库、技能、连接器、品牌、前台页面和评测集。
7. 模型可替换：LLM、Embedding、Rerank、VLM 等模型必须通过统一模型适配层接入，支持公有云 API、客户内网 OpenAI-compatible endpoint、本地开源模型服务和租户级模型路由。

## 3. 总体架构

```text
┌────────────────────────────────────────────────────────────────┐
│ Customer Apps                                                   │
│ 培训系统 / 知识库问答 / 客服辅助 / 研发助手 / 运营自动化          │
└────────────────────────────────────────────────────────────────┘
                                │
┌────────────────────────────────────────────────────────────────┐
│ App Template Layer                                              │
│ 标准前台模板 / 客户品牌 / 行业页面 / 业务工作台 / 管理后台         │
└────────────────────────────────────────────────────────────────┘
                                │
┌────────────────────────────────────────────────────────────────┐
│ AI Foundation Layer                                             │
│ Agent Runtime / Run Graph / RAG / Model / Tools / MCP           │
│ Connectors / Plugins / Triggers / Memory / Eval                 │
└────────────────────────────────────────────────────────────────┘
                                │
┌────────────────────────────────────────────────────────────────┐
│ Control Plane                                                   │
│ RBAC / Tenant / Audit / Config / Scheduler / File / Observability│
└────────────────────────────────────────────────────────────────┘
                                │
┌────────────────────────────────────────────────────────────────┐
│ Infrastructure                                                  │
│ PostgreSQL / Milvus / Object Storage / Queue / Parser Worker    │
│ Model Gateway / Model Runtime / Embedding / Rerank / Connector  │
└────────────────────────────────────────────────────────────────┘
```

### 3.1 Novex 在架构中的角色

Novex 不是单纯的后台管理模板，而是 AI 项目的控制平面：

- 管用户：租户、用户、角色、部门、菜单、数据权限、资源权限、身份提供商、外部账号绑定。
- 管资产：文件、知识库、文档、chunk、embedding、工具、技能、MCP server、连接器、插件、媒体资产。
- 管运行：Run Graph、Agent run、RAG run、工具调用、插件调用、触发器、任务队列、定时任务、审批、人机协同。
- 管质量：离线评测、线上反馈、回归测试、成本、延迟、命中率、引用准确率。
- 管交付：客户模板、行业模板、开通向导、品牌配置、环境配置、版本升级。

### 3.2 仓库和代码边界

第一阶段建议 Novex 保持一个 monorepo，不要过早拆成多个独立仓库。但代码边界必须从一开始拆清楚，不能把 AI 基建能力都堆进 `backend/src`。`backend` 应该承担 HTTP API、RBAC、租户、身份登录、配置、审计、调度和编排；Model、RAG、Agent、Run Graph、Tools、MCP、Connectors、Plugins、Triggers、Memory、Eval 这些可复用基建能力应放在独立 Rust workspace crates；MinerU、LibreOffice、OCR、部分本地模型 adapter 这类重依赖能力放在 worker 或 runtime service。

推荐目录结构：

```text
Novex/
  backend/                 Rust API，控制平面和业务编排
  crates/
    novex-ai-core/         Rust，AI 通用领域模型、Run Graph、Trace、Policy
    novex-model/           Rust，模型注册、能力描述、路由、适配器、用量和健康检查
    novex-rag/             Rust，chunk、embedding、检索、rerank、context builder
    novex-agent/           Rust，intent router、ReAct、planner、tool loop
    novex-tools/           Rust，tool registry、tool executor、工具权限策略
    novex-connectors/      Rust，GitHub、飞书、网页、数据库等连接器和凭据绑定
    novex-mcp/             Rust，MCP gateway、server/tool discovery、授权
    novex-plugin/          Rust，插件 manifest、安装、版本、权限、能力声明
    novex-trigger/         Rust，webhook、schedule、plugin event、外部事件路由
    novex-memory/          Rust，session/user/org/project memory
    novex-eval/            Rust，eval runner、指标、报告
    novex-trace/           Rust，agent trace、rollout bundle、replay summary、eval capture boundary
  services/
    parser-worker/         Python，MinerU、LibreOffice、OCR、格式转换
    model-runtime/         可选，Python 或独立进程，内网开源模型和本地 embedding/rerank adapter
    sandbox-runner/        Rust 优先，可选，代码/命令执行隔离
  admin/                   Next.js + TypeScript，管理后台
  apps/
    chat-web/              Next.js + TypeScript，默认 LLM Chat / 知识库问答前台
    training-web/          Next.js + TypeScript，员工培训模板
    agent-workspace/       Next.js + TypeScript，Agent 工作台模板
  templates/               客户交付模板、默认角色、默认 skills、默认 eval set
  infra/                   docker-compose、部署脚本、环境样例
  docs/                    架构、实施计划、交付手册
```

这个结构的原则是：仓库先不拆，模块先拆；部署先简单，服务边界先清楚。等某个模块需要独立团队、独立扩容、独立 SDK 或独立私有化交付时，再从 monorepo 中拆成独立仓库。

### 3.3 语言和运行时边界

Novex 的默认语言策略是 Rust first、Python sidecar、Next.js frontend。

Rust 负责所有需要长期稳定、强权限、强并发、强审计的核心能力：

- HTTP API 和 RBAC。
- tenant、ACL、policy、audit。
- identity provider、OAuth/OIDC 登录、外部账号绑定。
- model registry、model route、provider credential、capability policy。
- RAG 编排、检索、引用构建。
- Agent runtime、ReAct 状态机、intent router。
- tool registry、tool executor、MCP gateway。
- connector registry、plugin registry、trigger routing。
- memory policy 和 eval runner。
- scheduler、trace、成本、限流。

Python 只作为插件型运行时或 sidecar 使用，不进入核心控制面：

- MinerU。
- LibreOffice 转换调度。
- OCR。
- 文档版面分析。
- ML 生态依赖较重的 parser、rerank、本地 embedding adapter。
- 内网部署时对 vLLM、Ollama、TGI、llama.cpp、Xinference 等模型服务的轻量 adapter。
- 客户临时插件或实验性 connector。

前端统一使用 Next.js + TypeScript：

- `admin` 是平台管理后台。
- `apps/*` 是客户可交付前台模板。
- 默认模板、客户模板和行业模板都复用同一套前端组件、认证、权限和 API client。
