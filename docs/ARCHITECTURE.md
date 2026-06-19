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

跨语言调用必须通过稳定边界完成：HTTP、gRPC、queue job 或 MCP/tool schema。Python sidecar 和本地模型 adapter 不直接访问核心业务表，除非通过受控 API 或专用 job 表领取任务；解析结果、模型能力探测结果和用量结果必须写回标准结构，由 Rust backend 校验、入库和审计。

## 4. 与 Dify、FastGPT、Codex、Hive 的边界

### 4.1 Dify

Dify 强在应用搭建、工作流、工具、RAG、运营界面和模型供应商适配。它适合作为设计参考，也可以在早期作为外部 AI 应用运行时接入。但如果 Novex 要成为长期基座，不建议把 Dify 作为不可拆的底层核心，也不建议把可视化 Workflow Builder 作为 POC 主线。

推荐策略：

- POC 阶段可以参考 Dify 的概念：App、Dataset、Document、Tool、Provider、Conversation；Workflow 的暂停恢复、事件快照和节点级 trace 可以作为运行时机制参考，但不复制完整低代码流程画布产品。
- Novex 自己保留租户、权限、知识库资产、技能、工具策略、评测和交付控制。
- 如果客户已有 Dify，可以通过 adapter 调用 Dify app 或 workflow。
- 如果客户没有 Dify，Novex 应该能独立完成 RAG 和 Agent 的核心链路。

### 4.2 FastGPT

FastGPT 更偏知识库问答、流程编排、表单化配置和企业知识场景。它适合作为知识库产品交互参考。

推荐策略：

- 学习它的知识库管理、QA 测试、应用编排体验。
- 不把 FastGPT 作为核心依赖。
- Novex 的 RAG 模块要原生支持权限过滤、评测、引用回溯和客户模板。

### 4.3 Codex

Codex 的核心价值不是传统 RAG，而是 Agentic Code Search + Tool Use。它面对代码库时，不是预先把所有源码切 chunk 后向量召回，而是通过模型规划、文件搜索、`rg`、读取局部代码、执行命令、测试验证来逐步定位上下文。

推荐策略：

- Novex 的企业知识问答走 RAG。
- Novex 的代码库问答、研发助手和自动化任务走 Codex-like Agent Runtime。
- 为代码场景单独设计文件索引、符号索引、文本搜索、read range、命令执行、沙箱和审批。

### 4.4 Hive

Hive 代表多 Agent 协作思路。多 Agent 有价值，但不应该成为所有场景的默认形态。大多数企业问答、培训、客服和轻量自动化，一个主 Agent 加工具就够了。多 Agent 更适合长流程、强分工、高复杂度任务。

推荐策略：

- POC 默认单 Agent + 工具。
- 保留 supervisor-worker 编排能力。
- 多 Agent 只用于复杂交付，比如资料收集 agent、解析 agent、问答 agent、评测 agent、代码 agent 分工。

### 4.5 开发基准和架构守则

后续开发以 Novex 自身 RBAC 控制平面为地基，以 Codex 的 Agent Runtime 工程边界为主参考，以 Dify 和 FastGPT 的专项能力为辅参考。任何新模块、新表、新服务或新前端功能，都必须先按本节判断归属，避免把 Novex 做成 Dify/FastGPT 的二次封装，或把 Agent Runtime 做成散落在业务 service 里的临时代码。

参考优先级：

1. Novex 现有 RBAC 模板是系统事实源：账号、租户、角色、菜单、文件、配置、调度、审计和监控优先复用现有能力。
2. Codex 是 Agent Runtime 主参考：Run Graph、thread/turn/item 事件流、tool execution、MCP、sandbox、exec policy、file search、model provider、OTEL、crate 边界优先参考 Codex。
3. Dify 是插件和外部能力专项参考：plugin manifest、tool provider、datasource、trigger、OAuth、pause/resume、event snapshot 可以参考；Python monolith、完整 Workflow Builder、Dify 全家桶不作为 Novex 核心依赖。
4. FastGPT 是产品化控制面专项参考：知识库交互、团队权限、资源配额、sandbox provider、OpenAPI/outlink、eval、企业交付限制可以参考；Node.js 后端和应用搭建器路线不作为 Novex 基座主路径。

模块归属规则：

```text
用户 / 租户 / 权限 / 菜单 / 审计
  -> backend 现有 RBAC / system / monitor 模块扩展

模型注册 / 模型路由 / 模型凭据 / usage 归一化
  -> crates/novex-model

Run Graph / Run Step / Run Event / Pause / Cancel / Replay
  -> crates/novex-ai-core

Agent intent / planner / ReAct loop / tool loop
  -> crates/novex-agent，必须基于 novex-ai-core 的 run graph

知识库 / chunk / embedding / retrieval / rerank / citation
  -> crates/novex-rag，模型调用必须通过 novex-model

外部资源连接和同步
  -> crates/novex-connectors

Agent 可执行动作
  -> crates/novex-tools，必须有 tool schema、risk、permission、approval、audit

MCP server 接入
  -> crates/novex-mcp，统一 discovery、auth、secret、audit

可安装能力包
  -> crates/novex-plugin，声明 tools/connectors/triggers/oauth/ui/eval

外部事件入口
  -> crates/novex-trigger，统一 signature、idempotency、delivery、dead letter

文档解析 / OCR / MinerU / LibreOffice / 重 ML 依赖
  -> services/parser-worker，通过 job/API 边界写回结果

内网开源模型轻量适配
  -> services/model-runtime，可选，不直接访问核心业务表
```

不可违反的架构约束：

1. 核心控制面和 AI 基建默认 Rust；Python 只做 parser、ML adapter、实验性 connector 或插件 sidecar。
2. `backend` 只做 HTTP API、RBAC、租户、配置、审计和编排，不承载大块 RAG、Agent、Model、Tool 领域逻辑。
3. `crates/*` 不能反向依赖 `backend/src`；通用类型放 `novex-ai-core` 或对应领域 crate。
4. Sidecar 不能直接写核心业务表，必须通过受控 API、queue job 或专用 job 表交付结果。
5. 所有 AI 资源必须包含 `tenant_id`，可授权资源必须包含 `owner_id`、`visibility`、`acl_policy`。
6. 所有模型调用必须通过 `novex-model`，禁止在 RAG、Agent、Tool、Eval 中直接硬编码 provider SDK。
7. 所有运行态必须通过 `ai_run`、`ai_run_step`、`ai_run_event` 或等价结构记录，禁止只有日志没有可回放事件。
8. 所有外部动作必须进入 Tool Registry，声明 input/output schema、风险等级、权限码、审批策略、超时和成本。
9. 高风险工具默认需要 approval，尤其是写 GitHub、发外部消息、调用外部 POST、执行命令、修改客户业务数据。
10. GitHub 登录归 Identity Provider；GitHub repo/issue/PR 操作归 Connector + Tool；二者不能混用凭据。
11. 插件不是任意代码执行入口，必须有 manifest、版本、权限声明、安装记录、启用范围和审计。
12. Trigger 不是普通 HTTP 回调，必须有签名校验、幂等 key、路由目标、retry、dead letter 和 trace。
13. 文件、媒体和 tool 产物必须进入统一 file/asset 系统，访问必须有 scope、TTL、签名或权限校验。
14. POC 不做可视化 Workflow Builder，不默认做多 Agent，不默认启用 Milvus Cluster、K8s、GPU 常驻服务。

新增功能归属判断：

1. 如果功能改变“谁能访问什么”，先改 RBAC/ACL/permission，而不是在业务 service 写 if 判断。
2. 如果功能改变“模型怎么选”，先改 `novex-model`，再让 RAG/Agent/Tool 调用模型路由。
3. 如果功能改变“运行状态怎么流转”，先改 `novex-ai-core` 的 Run Graph，而不是在某个 API 里临时加状态。
4. 如果功能连接外部资源，先建 Connector；如果要让 Agent 执行动作，再把 Connector 能力暴露为 Tool。
5. 如果功能是可安装能力包，先建 Plugin manifest 和 permission grant，再暴露 tool/connector/trigger。
6. 如果功能从外部事件启动任务，先建 Trigger subscription 和 delivery log，再路由到 run/job/notification。
7. 如果功能只是客户差异，优先放模板、skill、connector 配置、model route 或 run graph policy，代码改动是最后手段。

依赖方向规则：

```text
backend
  -> novex-ai-core / novex-model / novex-rag / novex-agent / novex-tools / ...

novex-agent
  -> novex-ai-core / novex-model / novex-rag / novex-tools / novex-memory

novex-rag
  -> novex-ai-core / novex-model

novex-tools
  -> novex-ai-core / novex-model / novex-connectors / novex-mcp

novex-connectors
  -> novex-ai-core

novex-plugin
  -> novex-ai-core / novex-tools / novex-connectors / novex-trigger

novex-trigger
  -> novex-ai-core

services/*
  -> backend API / queue job / object storage，不直接依赖 backend 内部代码
```

架构变更门槛：

- 新增 crate、服务、核心表、跨语言调用方式、模型调用路径、权限模型、Run Graph 状态、Tool 风险等级、插件运行时类型时，必须先更新本架构文档或新增 ADR。
- 任何绕过 `novex-model`、`novex-ai-core`、Tool Registry、RBAC/ACL、secret 管理、trace/audit 的实现，都视为架构违规。
- POC 阶段允许能力很窄，但不允许绕过基建边界。可以少做功能，不能把模型、权限、运行态、工具治理写散。
- 每个里程碑完成前，必须检查本节规则：模块归属、依赖方向、数据模型、权限、审计、trace、eval 是否满足。

## 5. 后端模块规划

建议在现有 Rust backend 中新增轻量 `ai` 业务入口，但不要把 AI 基建都写进 `backend/src`。`backend` 负责控制面和 API 编排，通用 AI 能力放在 `crates/`，重依赖解析能力放在 `services/parser-worker/`。

```text
backend/src/domain/ai/
  app/             AI 应用、模板、发布版本、渠道配置
  tenant/          租户、客户、空间、资源归属
  acl/             AI 资源 ACL、可见性、授权策略

backend/src/domain/identity/
  provider/        GitHub、OIDC、SAML、企业微信等身份提供商
  account/         外部账号绑定、解绑、租户准入策略

backend/src/application/ai/
  app_service.rs       应用配置、发布、模板绑定
  model_api.rs         模型供应商、模型部署、模型路由和租户配置
  knowledge_api.rs     知识库 API 编排，调用 novex-rag
  agent_api.rs         Agent API 编排，调用 novex-agent
  tool_api.rs          工具 API 编排，调用 novex-tools
  connector_api.rs     连接器配置、凭据绑定、数据源同步
  plugin_api.rs        插件安装、启用、版本、权限和能力声明
  trigger_api.rs       webhook、schedule、plugin event 触发器
  eval_api.rs          评测 API 编排，调用 novex-eval
  delivery_service.rs  客户模板初始化和交付包导入导出

backend/src/application/identity/
  oauth_service.rs     OAuth/OIDC 登录、回调、账号绑定和解绑
  provider_service.rs  身份提供商配置、租户准入和安全策略

crates/novex-ai-core/
  module.rs        FoundationModule、FoundationStatus、foundation module skeleton catalog
  context.rs       TenantContext、ResourceRef
  integration_usage.rs  external integration usage subject、window、limit enforcement
  run_graph.rs     RunStatus、run transition、step/event/pause vocabulary
  budget.rs        TaskBudget、POC/default budget limits、normalization
  lib.rs           crate facade 和 CRATE_ID

crates/novex-model/
  taxonomy.rs      model kind、provider type、route purpose、runtime target
  route.rs         runtime route/config/summary、env-backed route construction
  provider.rs      provider-neutral stream/media/rerank/embedding DTO
  usage.rs         token usage normalization、text token estimation
  cost.rs          usage cost input、cost estimation
  policy.rs        route fallback policy evaluation
  util.rs          JSON field parsing、URL/key helpers、registry token normalization
  lib.rs           crate facade 和 FoundationModule constructor

crates/novex-rag/
  knowledge/       dataset、document、chunk、citation
  ingest/          ingestion pipeline、chunk strategy
  retrieval/       vector search、keyword search、hybrid retrieval
  context/         context builder、citation builder

crates/novex-agent/
  intent.rs        AgentIntent、intent routing
  tool_selection.rs  SelectedTool、seeded POC tool selection、tool policy mapping
  plan.rs          AgentLoopKind、AgentRunPlan、budget/memory-aware ReAct planning
  text.rs          shared text matching helper
  module.rs        FoundationModule constructor
  lib.rs           crate facade 和 CRATE_ID

crates/novex-tools/
  types.rs         tool kind、risk、approval policy、definition、execution envelope
  policy.rs        tool risk / approval policy evaluation
  concurrency.rs   shared/exclusive lock、parallel batch planning
  executor.rs      executor binding、dispatch plan、registry、registry errors
  router.rs        model-visible tool spec、tool router、routed tool call
  definitions.rs   built-in agent model-loop / customer-service tool definitions
  adapters.rs      Feishu、GitHub、media tool input normalization
  media.rs         image request/result DTO、provider response parsing
  lib.rs           crate facade 和 FoundationModule constructor

crates/novex-connectors/
  kind.rs          ConnectorKind connector vocabulary
  credential.rs    CredentialScope、credential binding/source/resolution helpers
  feishu.rs        Feishu webhook text message DTO
  github.rs        GitHub code search/read request DTO、response parser
  module.rs        FoundationModule constructor
  lib.rs           crate facade 和 CRATE_ID

crates/novex-mcp/
  gateway/         MCP server 注册、tool discovery、调用代理
  auth/            租户级授权、secret、连接状态

crates/novex-plugin/
  types.rs         PluginRuntime、capability、manifest、network policy、validation error
  validation.rs    manifest validation、required permission extraction
  builtin.rs       built-in plugin manifest catalog
  module.rs        FoundationModule constructor
  lib.rs           crate facade 和 CRATE_ID

crates/novex-trigger/
  types.rs         TriggerSourceKind、TriggerTargetKind
  delivery.rs      delivery target validation、retry policy、route snapshot planning
  webhook.rs       webhook signature verification、idempotency validation
  module.rs        FoundationModule constructor
  lib.rs           crate facade 和 CRATE_ID

crates/novex-memory/
  policy/          写入策略、TTL、脱敏、删除
  store/           session/user/org/project memory
  retrieval/       memory retrieval + RBAC filter

crates/novex-eval/
  case.rs          eval target/metric、case input/expected/actual、candidate DTO
  trace_extract.rs trace bundle -> eval candidate/actual、trace summary helpers
  score.rs         metric dispatch、RAG/intent/tool/customer-service/cost/latency scoring
  report.rs        regression report aggregation
  text.rs          case-insensitive match、score rounding helpers
  lib.rs           crate facade 和 FoundationModule constructor

crates/novex-trace/
  event.rs         TraceEventKind、TraceEvent constructors
  bundle.rs        TraceBundle ordering、tool-call count、replay summary derivation
  summary.rs       TraceReplaySummary DTO
  module.rs        FoundationModule constructor
  lib.rs           crate facade 和 CRATE_ID

services/parser-worker/    Python
  MinerU                   PDF、扫描件、复杂版面解析
  LibreOffice              Office 转 PDF
  OCR                      图片和扫描件文字识别
  normalizer               markdown、html、txt、csv、xlsx、code 原生解析

services/model-runtime/    可选
  llm-adapter              Qwen、DeepSeek、Gemma、Llama 等内网模型服务适配
  embedding-adapter        bge、gte、qwen embedding、jina embedding 等向量模型适配
  rerank-adapter           bge-reranker、jina reranker、gte reranker 等重排模型适配
  openai-compatible-proxy  将内网模型统一暴露成 OpenAI-compatible API
```

### 5.1 控制平面复用现有能力

已有模块可以直接作为 AI 基座的控制平面：

- `auth`: 登录、JWT、客户端认证。
- `rbac`: 用户、角色、菜单、权限码。
- `data_scope`: 部门和数据权限。
- `system/file`: 文件管理和存储配置。
- `scheduler`: 异步任务、定时任务、队列执行。
- `monitor`: 日志和在线用户。

AI 模块新增资源时，必须复用这些基础能力：

- 所有 API 都挂权限码，例如 `ai:knowledge:list`、`ai:agent:run`、`ai:tool:invoke`。
- 所有 AI 资产都包含 `tenant_id`、`owner_id`、`visibility`、`acl_policy`。
- 所有工具调用都写入审计日志和 trace。

### 5.2 模型适配层

模型适配必须作为独立基础模块处理，不能散落在 RAG、Agent、Tools 或业务 service 中。Novex 后续会面对公有云模型、客户内网模型、离线私有化模型和混合部署，如果模型配置只写在某个问答接口里，后续接 Qwen、DeepSeek、Gemma、私有 embedding、私有 rerank 时会快速失控。

`novex-model` 负责统一抽象以下模型类型：

- LLM: chat、completion、reasoning、code、tool calling、JSON mode、streaming。
- Embedding: text embedding、multilingual embedding、image embedding，记录维度、归一化策略和 batch 限制。
- Rerank: cross-encoder rerank、listwise rerank，记录 score 方向、top_n、最大候选数和输入长度。
- VLM: 图片理解、文档截图理解、图片 caption，用于多模态检索和训练材料解析。
- ASR / TTS: 可以后置，但模型注册和路由结构要预留。

模型接入按 provider、deployment、model profile 三层管理：

```text
Provider
  OpenAI / Azure OpenAI / DashScope / DeepSeek / OpenRouter / local-openai-compatible
Deployment
  public-api / private-vllm / private-ollama / private-tgi / private-xinference
Model Profile
  qwen3-32b-instruct / deepseek-r1 / gemma-4 / bge-m3 / bge-reranker-v2
```

每个模型配置至少包含：

- `provider_type`: openai-compatible、dashscope、deepseek、azure-openai、local-runtime。
- `endpoint`: base URL、API path、network zone、内外网标识。
- `credential_ref`: 租户级或平台级密钥引用，密钥必须加密存储。
- `model_name`: 供应商模型名或内网部署名。
- `model_kind`: llm、embedding、rerank、vlm、asr、tts。
- `capabilities`: streaming、tool_calling、json_mode、vision、reasoning、function_schema。
- `limits`: context window、max output tokens、batch size、timeout、QPS、并发。
- `embedding_spec`: dimension、normalize、pooling、distance metric。
- `rerank_spec`: max candidates、top_n、score range、score higher is better。
- `cost_spec`: 按 token、字符、请求、向量条数或私有化固定成本估算。
- `fallback_policy`: 降级模型、重试次数、熔断时间、是否允许跨网络区域降级。

模型调用链路统一为：

```text
业务模块
  -> Model Route Resolve
  -> Policy Check
  -> Provider Adapter
  -> Model Runtime / Public API
  -> Usage Normalize
  -> Trace + Cost + Health Update
```

RAG、Agent、Eval 都只依赖 `novex-model` 的 trait 和路由结果，不直接关心具体供应商。内网部署时，Qwen、DeepSeek、Gemma、bge embedding、bge reranker 等模型优先通过 OpenAI-compatible endpoint 或 `model-runtime` 暴露成统一协议；只有供应商协议差异明显时才写专用 adapter。

模型路由必须支持租户级配置：

- 平台默认模型：没有客户配置时使用。
- 租户默认模型：客户自己的 LLM、embedding、rerank、VLM。
- 应用级模型：某个 AI app 指定模型。
- 技能级模型：某个 skill 指定强推理或低成本模型。
- 任务级模型：RAG answer、query rewrite、embedding、rerank、eval judge、code agent 分别配置。

私有化部署要重点保证：

- 模型 endpoint 可以只在内网访问。
- secret 不出租户或部署边界。
- embedding 维度变更必须触发索引兼容检查，不能把不同维度写入同一向量列。
- rerank 分数语义必须归一化，否则评测和阈值不可比较。
- 模型能力必须可探测、可手工覆盖、可审计。

## 6. 前端模块规划

Admin 端继续使用当前 Next.js 后台模板，新增 AI 管理菜单：

```text
admin/app/(main)/ai/
  dashboard/       AI 基座总览：调用量、成本、质量、失败率
  apps/            AI 应用管理：前台模板、发布配置、渠道
  models/          模型供应商、内网模型、embedding、rerank、路由和健康检查
  knowledge/       知识库：上传、解析、切片、检索测试、引用预览
  agents/          Agent：提示词、模型、工具、技能、记忆、策略
  skills/          Skills：技能包、版本、启用范围、评测
  tools/           Tools：工具注册、schema、密钥、权限、审计
  mcp/             MCP server 管理、连接状态、工具发现
  connectors/      飞书、GitHub、网页、数据库等连接器
  plugins/         插件安装、启用、权限、版本、能力声明
  triggers/        webhook、schedule、GitHub event、plugin event
  media/           图片生成、文件资产、媒体任务和引用
  memory/          记忆策略、用户记忆、组织记忆、清理规则
  evals/           评测集、评测运行、回归报告
  traces/          Agent run、RAG run、tool call trace
  delivery/        客户模板、开通向导、项目交付清单
```

身份提供商不放在 AI 菜单下，应作为系统安全能力进入系统管理：

```text
admin/app/(main)/system/identity/
  providers/       GitHub、OIDC、SAML、企业微信等登录源
  accounts/        外部账号绑定、解绑、审计
  policies/        租户准入、域名限制、默认角色、自动加入策略
```

前台模板必须独立于 Admin，但共享认证、权限和 API。Admin 是控制平面，不是客户使用 Novex 的主要入口；客户和员工看到的应是业务应用，而不是后台菜单。

```text
apps/
  training-web/    员工培训前台
  chat-web/        知识库问答前台
  agent-workspace/ Agent 工作台
```

POC 阶段不再把核心演示放在 `admin` 内。`admin` 可以保留 smoke test、配置和治理页面，但客户演示、员工使用、模型对话、知识库问答、工作流运行和 Agent 工作台都应优先落在 `apps/*`。这样 POC 才能证明 Novex 是可交付的 AI 应用基座，而不是一个 AI 后台管理系统。

### 6.1 默认应用模板

Novex 必须内置默认模板，否则基座能力无法快速交付给客户。默认模板不是 demo，而是可复制、可配置、可评测、可发布的应用起点。

第一批默认模板：

1. LLM Chat 模板：纯大模型问答，不接知识库。适合通用助手、写作、头脑风暴、简单客服。
2. Knowledge Base Chat 模板：知识库问答，走 RAG，答案带引用。适合企业制度、培训资料、产品文档、FAQ。
3. Agent Workspace 模板：带 tools、skills、memory、MCP，可以执行任务。适合飞书、GitHub、图片生成、自动化操作。
4. Training App 模板：知识库问答 + 自动出题 + 学习记录 + 飞书通知，是第一个 POC 客户样板。

模板必须包含：

- 前台页面。
- 默认菜单。
- 默认角色。
- 默认权限码。
- 默认 prompt。
- 默认 skill。
- 默认工具配置。
- 默认 eval set。
- 默认品牌配置项。
- 默认前台路由和页面布局。
- 默认发布配置和入口 URL。

客户交付流程应从选择模板开始：

```text
选择默认模板
  -> 创建租户
  -> 初始化角色和菜单
  -> 配置模型和知识库
  -> 启用 skills/tools/connectors
  -> 配置品牌和前台入口
  -> 跑模板评测集
  -> 发布客户应用
```

### 6.2 C 端应用体验边界

`apps/*` 面向业务用户、客户管理员和交付验收，不复刻 Admin 的表格后台。页面形态可以参考 FastGPT 和 Dify 的应用工作台经验：左侧是应用和能力导航，中间是主要任务区，右侧是上下文、引用、运行状态、变量、trace 或发布信息。Novex 可以学习这种工作台式体验，但不在 POC 阶段复制完整低代码 Workflow Builder。

C 端应用应承担这些体验：

```text
apps/chat-web/
  chat/            模型对话，支持模型路由、会话历史、文件上下文
  knowledge/       知识库问答，答案带引用、检索命中、反馈
  share/           可发布的公开或半公开访问入口，受 API key / public link 限流

apps/training-web/
  learn/           员工培训首页、任务、进度、薄弱知识点
  ask/             基于培训资料的知识库问答
  quiz/            自动出题、答题、解析、错题回顾
  records/         员工学习记录和答题记录
  notifications/   飞书等通知状态

apps/agent-workspace/
  runs/            Agent run 列表、状态、暂停恢复、人工确认
  workflow/        工作流/任务执行视图，展示步骤、变量、事件和结果
  tools/           可用工具、权限提示、调用结果
  memory/          当前会话和项目记忆
```

这些页面可以调用同一套 backend API，但交互目标不同：

- Admin 配置模型、知识库、技能、工具、权限、评测和发布。
- C 端执行对话、问答、训练、任务和 Agent run。
- Admin 看到治理和审计字段；C 端只看到业务必需字段、引用、状态和可解释结果。
- C 端所有写操作仍然走 RBAC、租户、资源 ACL、限流、审计和 trace。

### 6.3 POC 前台设计优先级

第一个 POC 前台必须是 `apps/training-web`。原因是它能同时验证知识库、模型路由、RAG 引用、自动出题、学习记录、通知工具、评测集和客户模板，比单纯 chat 更能证明 Novex 的基座价值。

POC 页面风格要求：

- 不做营销 landing page，打开就是可用的培训工作台。
- 页面要有真实业务密度：任务、资料、问答、测验、进度和引用。
- 允许参考 FastGPT/Dify 的侧边导航、会话面板、知识库问答、应用发布和运行状态布局。
- Workflow 在 POC 中只做“任务/Agent run 执行视图”，不做拖拽式低代码编排器。
- 模型对话、知识库问答、培训测验和 Agent run 都在 C 端完成；admin 只配置和观察。

## 7. 核心链路

### 7.1 知识库 RAG 链路

```text
上传文件
  -> 存原始文件
  -> 创建解析任务
  -> 格式识别
  -> 转换或直接解析
  -> MinerU / parser worker 输出结构化文档
  -> chunk
  -> resolve embedding model
  -> embedding
  -> 写入 Milvus collection
  -> 检索测试
  -> 发布知识库
  -> 用户提问
  -> 权限过滤
  -> resolve retrieval / rerank / answer model
  -> hybrid retrieval
  -> rerank
  -> context builder
  -> LLM answer
  -> 引用页码、段落、原文
  -> trace + eval
```

RAG 必须做到：

- 按租户、知识库、文档、角色、部门过滤。
- 支持向量检索和关键词检索混合召回。
- 向量检索默认使用 Milvus；PostgreSQL 只保存知识库、文档、chunk、权限、索引配置和运行元数据。
- 支持 rerank，提高答案上下文质量。
- 答案必须带引用，引用可回到文档页码、段落、bbox 或 chunk。
- 每次回答记录检索命中、rerank 分数、上下文 token、模型输出和用户反馈。

### 7.2 文件解析链路

推荐保留原文件，同时生成标准化中间件。

```text
Original File
  -> Asset Store
  -> Normalizer
     - PDF: 直接进入 MinerU
     - Office: LibreOffice 转 PDF，再进入 MinerU
     - Image: OCR / MinerU
     - HTML / Markdown / TXT: 原生解析
     - CSV / XLSX: 表格解析，不强制转 PDF
     - Code / Log / JSON: 原生解析，不转 PDF
  -> Structured Document
  -> Chunk
```

MinerU 可以作为 PDF、扫描件、复杂版面和表格公式的主解析器。用户提出的“其他格式先转 PDF 再用 MinerU”适合作为 Office 文档的主路径，但不建议覆盖所有格式。CSV、代码、日志、Markdown、HTML 这类文本结构强的文件，原生解析更准、成本更低。

### 7.3 Agent Runtime / Run Graph 链路

Novex 不把可视化 Workflow Builder 作为第一阶段核心产品形态。现在 Agent 的主流方向是让模型动态规划和调用工具，手工拖拽大量固定节点的低代码工作流会逐步退到高级自动化场景。但这不代表运行时编排会消失。Agent、RAG、Trigger、异步 Job、人工审批和后续可选的 Flow Builder，都需要共享一套底层 Run Graph。

Run Graph 是后端运行时机制，不等同于用户可见的流程画布产品：

- Agent 负责动态规划、工具选择和上下文决策。
- Run Graph 负责状态、步骤、事件、暂停、恢复、取消、重试、幂等、审计和回放。
- 确定性流程可以固化为 run step；不确定步骤交给 Agent loop。
- 可视化 Flow Builder 只作为后续高级能力，不进入 POC 必做范围。

```text
用户输入 / trigger / job
  -> create run
  -> build run graph
  -> Policy Check / Permission Profile
  -> Model Route Resolve
  -> Context Builder
  -> Planner / ReAct Loop
  -> Tool Selector
  -> Tool Executor / MCP Gateway
  -> Observation
  -> run step event
  -> Continue / Pause / Final Answer
  -> Trace / Eval / Feedback / Replay
```

Agent Runtime 需要支持：

- ReAct: 思考、行动、观察、继续或结束。
- 意图预测: 问答、检索、工具调用、任务执行、代码库搜索、培训测验、人工转接。
- Run 状态机: `queued`、`running`、`waiting_approval`、`paused`、`resuming`、`cancelling`、`cancelled`、`failed`、`succeeded`。
- Run Step: model call、tool call、retrieval、rerank、approval、human input、connector sync、media job 等步骤必须有统一结构。
- Run Event: 每次输入、输出、状态变化、错误、重试、审批、暂停和恢复都写入可回放事件日志。
- 工具预算: 最大轮次、最大成本、最大耗时、最大工具调用次数。
- 审批: 高风险工具需要人确认，例如发消息、写 GitHub、改数据、执行命令。
- 可恢复: 长任务可以保存快照，由 scheduler 或 worker 继续执行；用户断线后可以从事件快照恢复 UI。
- 可回放: 每一步模型输入、工具输入、工具输出、错误和决策都可回放。
- 可取消: 用户或系统策略可以中断 run，后续 step 不得继续产生副作用。
- 人机协同: human input 和 approval 统一建模为 pause reason，恢复时携带 resume token 和输入 payload。

这层设计吸收 Dify 的暂停恢复和事件快照经验、FastGPT 的循环/并发/沙箱限制经验，以及 Codex 的 thread/turn/item 事件流经验，但 Novex 不把“画工作流”当成所有 AI 应用的入口。

### 7.4 Code Agent 链路

代码类能力不要直接套普通 RAG。建议单独实现 Code Workspace：

```text
Repo Connector
  -> 文件列表索引
  -> 符号索引
  -> lexical search
  -> read range
  -> dependency / import graph
  -> command sandbox
  -> test runner
  -> patch proposal
```

适用场景：

- 代码库问答。
- PR 辅助。
- 自动定位 bug。
- 生成小改动方案。
- 客户私有系统的研发助手。

POC 可以先做 GitHub connector + repo search + read range，不急着做自动改代码。

## 8. Skills 设计

Skill 是可复用能力包，不只是 prompt。一个 skill 应该包含：

```text
skill.json
  id
  name
  version
  description
  instructions
  input_schema
  output_schema
  required_tools
  required_mcp_servers
  permission_codes
  eval_cases
  examples
```

Skill 的典型例子：

- 员工培训出题。
- 培训资料问答。
- 飞书通知。
- GitHub issue 总结。
- 图片生成。
- 合同审查。
- SOP 生成。
- 客服质检。

Skill 必须可版本化、可灰度、可评测、可按租户启用。客户定制优先做 skill 配置，不优先改核心代码。

## 9. Memory 设计

Memory 分四层：

1. Session Memory: 当前会话内短期上下文，默认随会话结束清理。
2. User Memory: 用户偏好、常用上下文、历史任务摘要，需要用户授权和可清除。
3. Org Memory: 企业级知识、术语、流程、业务规则，属于租户资产。
4. Project Memory: 某个客户项目、某个应用、某个交付版本的长期背景。

Memory 写入必须有策略：

- 哪些内容可以自动写入。
- 哪些内容需要用户确认。
- 哪些内容必须禁止写入。
- TTL、脱敏、加密、审计和删除机制。

Memory 检索必须走权限过滤，不能因为记忆系统绕开 RBAC。

## 10. Identity、Connectors、Tools、MCP、Plugins 和 Triggers 设计

这些能力必须分层，不能全部叫“插件”。边界如下：

- Identity Provider: 解决“用户是谁”，例如 GitHub 登录、OIDC、SAML、企业微信登录。
- Connector: 解决“如何连接外部资源”，例如 GitHub repo、飞书文档、网页、数据库。
- Tool: 解决“Agent 可以执行什么动作”，例如画图、发飞书消息、读 GitHub issue、调用 HTTP webhook。
- MCP Gateway: 解决“如何接入 MCP server 暴露的工具和资源”。
- Plugin: 解决“如何安装一个能力包”，一个插件可以声明 tools、connectors、triggers、OAuth clients、UI config 和 eval cases。
- Trigger: 解决“外部事件如何进入 Novex”，例如 GitHub webhook、schedule、plugin event。

GitHub 登录属于 Identity Provider，不属于 GitHub connector。GitHub repo search/read、issue/PR 读写属于 Connector + Tool。图片生成属于 Media Tool，底层可以调用模型适配层或外部图片服务。Dify 的 plugin/tool 思路可以参考，但 Novex 必须保留自己的租户、RBAC、密钥、审批、审计和 trace。

### 10.1 Tool Registry

Tool Registry 负责管理工具定义：

- tool id、名称、描述。
- input schema、output schema。
- tool type: HTTP、function、MCP、connector、sandbox、model。
- 权限码。
- 租户启用范围。
- 风险等级。
- 审批策略。
- 调用成本和超时。

高风险工具必须显式声明风险等级和审批策略。示例：

- 低风险：知识库检索、文档引用读取、GitHub repo read。
- 中风险：图片生成、网页抓取、外部 HTTP GET。
- 高风险：发飞书消息、写 GitHub issue / PR、调用外部 POST、执行命令、修改客户业务数据。

### 10.2 Media / Image Tools

画图、图片生成、图片编辑和视觉理解应归为 Media Tool，而不是独立散落在业务代码中。

推荐链路：

```text
用户请求
  -> Agent / Skill
  -> Tool Policy Check
  -> media tool call
  -> Model Route Resolve 或 external image provider
  -> async media job
  -> asset store / sys_file
  -> tool result: asset_id、preview_url、metadata
  -> trace + audit + eval feedback
```

Media Tool 必须支持：

- 同步或异步执行，默认异步。
- 生成结果进入统一文件/资产系统。
- 记录 prompt、模型、尺寸、风格、seed、成本、耗时。
- 对敏感内容、版权风险和客户数据泄露做策略控制。
- 在客户模板中配置默认图片模型，例如外部图片 API、内网 ComfyUI、Stable Diffusion、Qwen image 兼容服务等。

### 10.3 Connectors

Connector 负责外部资源连接和凭据绑定，不直接代表一个 Agent tool。一个 connector 可以同时服务 RAG 数据源、Agent tool 和 trigger。

第一批 connector：

- GitHub: repo list、repo search、read file、issue/PR read，写操作后置并默认高风险审批。
- 飞书: 文档读取、消息发送、群通知。
- Web: sitemap、网页抓取、定期同步。
- Database: 只读查询优先，写操作必须后置并审批。
- Object Storage: S3/OSS/MinIO 文件同步。

Connector 凭据必须区分 scope：

- platform scope: 平台统一配置。
- tenant scope: 某个租户共享。
- user scope: 用户授权，例如 GitHub OAuth。
- app scope: 某个 AI app 专用。

GitHub 登录和 GitHub connector 必须解耦。用户用 GitHub 登录 Novex，不代表 Novex 自动获得其 repo 权限；访问 repo 需要单独 OAuth scope、GitHub App installation 或 token 绑定。

### 10.4 MCP Gateway

MCP Gateway 负责统一接入外部 MCP server：

- server 注册。
- tool discovery。
- secret 管理。
- per-tenant 授权。
- 网络出口 allow-list。
- 工具调用审计。
- 失败重试和熔断。

### 10.5 Plugin System

Plugin 是可安装能力包，不是任意代码执行入口。插件 manifest 应声明能力和权限，由 Novex 控制面审核、安装、启用和审计。

```text
plugin.json
  id
  name
  version
  publisher
  description
  capabilities
    tools[]
    connectors[]
    triggers[]
    oauth_clients[]
    ui_config_schema
    eval_cases[]
  permissions
    permission_codes[]
    oauth_scopes[]
    network_access[]
    secret_refs[]
  runtime
    hosted_http
    mcp_server
    builtin_adapter
```

插件生命周期：

```text
marketplace / local package
  -> manifest validate
  -> permission review
  -> install
  -> tenant enable
  -> credential bind
  -> tool / connector / trigger discovery
  -> eval smoke test
  -> publish to app / skill
```

POC 阶段不建议实现完整插件市场，但要把 manifest、权限声明、安装记录、启用范围和插件能力表设计好。早期可以先支持内置插件和本地插件包。

### 10.6 Triggers

Trigger 负责外部事件进入系统：

- webhook: GitHub、飞书、客户系统回调。
- schedule: 定时任务，复用现有 scheduler。
- plugin event: 插件自定义事件。
- connector event: 数据源同步完成、文档更新、issue 变更。

Trigger 必须支持：

- endpoint 签名校验。
- 幂等 key。
- 租户和应用路由。
- 失败重试和死信。
- 触发目标：run graph、agent run、job、notification；后续可扩展到可视化 flow run。
- trace 和审计。

GitHub webhook 示例：

```text
GitHub webhook
  -> signature verify
  -> tenant/app route
  -> trigger policy check
  -> create job or agent run
  -> trace + audit
```

### 10.7 第一批工具和插件能力

POC 建议优先做：

- 知识库检索工具。
- 文档引用读取工具。
- 飞书发送消息工具。
- GitHub repo search/read 工具。
- 图片生成工具。
- HTTP webhook 工具。
- GitHub identity provider。
- GitHub connector POC。
- 本地插件 manifest POC。

不要在 POC 阶段一次性做太多连接器和插件市场。先把工具治理、身份边界、连接器凭据、插件权限、触发器、审计和 trace 打通。

## 11. 多 Agent 策略

多 Agent 有意义，但不应是默认复杂度。推荐三档：

1. 单 Agent: 默认形态，一个主 Agent 使用 RAG、tools、memory、skills。
2. Supervisor + Workers: 复杂任务由 supervisor 拆分，worker 分别执行检索、解析、写作、评测、通知。
3. Run Graph + Agent 混合: 可确定流程固化为 run step，不确定步骤交给 Agent 动态规划。

适合多 Agent 的场景：

- 多资料源调研。
- 长周期企业流程自动化。
- 复杂代码库分析。
- 培训内容生成、审核、出题、发布、通知的流水线。

不适合多 Agent 的场景：

- 普通知识库问答。
- 单轮客服辅助。
- 简单表单填充。
- 权限敏感但流程短的后台操作。

## 12. 评测体系

评测是基座的一等模块，不是上线后的补丁。

### 12.1 离线评测

每个客户模板都应带最小评测集：

- RAG 问答：问题、期望答案、期望引用、禁止答案。
- 检索：目标文档、目标 chunk、top-k 命中率。
- 重排：rerank 后目标 chunk 排名。
- 意图识别：输入、期望 intent、期望 route。
- 工具调用：期望工具、参数断言、风险策略。
- ReAct：最大步数、是否触发审批、是否完成目标。
- 安全：越权访问、敏感信息、prompt injection。

核心指标：

- retrieval recall@k
- citation accuracy
- answer faithfulness
- intent accuracy
- tool selection accuracy
- task success rate
- hallucination rate
- cost per answer
- latency p50 / p95

### 12.2 线上评测

线上每次运行写入 trace：

- 输入。
- 权限上下文。
- 模型路由结果。
- 检索 query。
- 命中 chunk。
- rerank 分数。
- prompt 版本。
- 模型和参数。
- 工具调用。
- 输出。
- 用户反馈。
- 成本和耗时。

线上反馈进入 eval dataset，形成回归集。每次变更 prompt、chunk 策略、embedding、rerank、model、tool schema，都必须能跑回归。

### 12.3 CI Gate

建议从 POC 开始建立最小 gate：

- 单测必须通过。
- RAG smoke eval 必须通过。
- 工具权限越权测试必须通过。
- 核心 intent eval 不低于阈值。
- prompt 或 parser 改动必须生成评测报告。

## 13. 数据模型草案

核心表建议：

```text
sys_tenant
sys_tenant_user
sys_tenant_role
sys_member_group
sys_member_group_user
sys_resource_permission
sys_quota_policy
sys_usage_meter
sys_rate_limit_policy
sys_identity_provider
sys_external_account
sys_oauth_state
sys_secret
ai_app
ai_app_release
ai_template
ai_model_provider
ai_model_deployment
ai_model_profile
ai_model_credential
ai_model_route
ai_model_health_check
ai_model_usage
ai_api_key
ai_public_link
ai_vector_collection
ai_dataset
ai_document
ai_document_asset
ai_parse_job
ai_chunk
ai_embedding
ai_rag_run
ai_run
ai_run_step
ai_run_event
ai_run_pause
ai_agent
ai_agent_run
ai_agent_trace
ai_skill
ai_skill_version
ai_tool
ai_tool_call
ai_mcp_server
ai_mcp_tool
ai_connector
ai_connector_credential
ai_connector_sync_job
ai_plugin
ai_plugin_version
ai_plugin_installation
ai_plugin_capability
ai_trigger
ai_trigger_event
ai_media_job
ai_media_asset
ai_memory
ai_eval_dataset
ai_eval_case
ai_eval_run
ai_eval_result
```

关键字段约束：

- 所有业务资源包含 `tenant_id`。
- 租户是控制平面资源，不建议做只服务 AI 的孤立 `ai_tenant`；AI 表统一引用控制平面的租户 ID。
- 可授权资源包含 `owner_id`、`visibility`、`acl_policy`。
- 资源授权统一通过 `sys_resource_permission` 表表达，支持 user、role、group、org 四类 subject，支持 owner、manage、write、read 等权限值，支持父子资源继承和显式覆盖。
- `sys_member_group` 和 `sys_member_group_user` 用于团队协作、共享知识库、共享 app、共享 connector credential，不应只依赖角色权限。
- `sys_quota_policy`、`sys_usage_meter`、`sys_rate_limit_policy` 统一约束租户、应用、模型、工具、向量索引、sandbox、插件调用等资源，避免用量控制散落在业务代码中。
- `sys_secret` 必须包含 `scope_type`、`scope_id`、`key_version`、`ciphertext`、`masked_value`、`expires_at`、`rotated_at`、`last_used_at`，并记录创建人和审计信息。
- 可版本资源包含 `version`、`status`、`published_at`。
- 运行类资源包含 `trace_id`、`cost`、`latency_ms`、`model`、`prompt_version`。
- `ai_run` 是 Agent、RAG、Trigger、Job 共享的运行实例，包含 `run_type`、`status`、`source_type`、`source_id`、`tenant_id`、`app_id`、`created_by`、`budget_policy`、`started_at`、`finished_at`。
- `ai_run_step` 是 Run Graph 的步骤节点，包含 `run_id`、`parent_step_id`、`step_type`、`status`、`input_ref`、`output_ref`、`tool_call_id`、`model_profile_id`、`retry_count`、`cost`、`latency_ms`。
- `ai_run_event` 是可回放事件日志，包含 `run_id`、`step_id`、`event_type`、`sequence_no`、`payload_ref`、`created_at`，用于 UI 断线恢复、审计和重放。
- `ai_run_pause` 记录 human input、approval、external callback 等暂停原因，包含 `run_id`、`step_id`、`pause_reason`、`requested_input_schema`、`resume_token_hash`、`expires_at`、`resumed_at`。
- `ai_rag_run`、`ai_agent_run`、`ai_agent_trace` 只能作为 `ai_run` 的场景化摘要、索引或兼容视图，必须引用 `run_id`，不能形成第二套独立运行状态机。
- 文档和 chunk 包含 `source_uri`、`page_no`、`bbox`、`section_path`、`hash`。
- 模型配置包含 `provider_type`、`endpoint`、`model_kind`、`capabilities`、`limits`、`credential_ref`、`network_zone`。
- embedding 记录必须包含 `model_profile_id`、`dimension`、`distance_metric`、`normalized`，同一个向量索引不能混写不兼容维度。
- `ai_vector_collection` 管理 Milvus collection / index 元数据，包含 `collection_name`、`model_profile_id`、`dimension`、`metric_type`、`index_type`、`partition_strategy`、`consistency_level`、`schema_version`、`status`。
- Milvus collection 建议按 embedding 维度、metric、模型 profile 分组，不建议按每个文档建 collection。租户、dataset、document、chunk、权限过滤所需字段应作为 scalar field 或 PostgreSQL 预过滤条件参与检索。
- 大规模数据场景下，Milvus 保存向量和检索必要 scalar metadata，PostgreSQL 保存完整 chunk 文本、引用、权限、审计和业务元数据。检索结果必须回查 PostgreSQL 做最终权限校验和引用构建。
- rerank 运行记录必须包含 `model_profile_id`、`top_n`、`score_range`、`score_direction`，避免不同 rerank 模型的分数阈值混用。
- 身份提供商属于系统安全资源，包含 `provider_type`、`client_id`、`secret_ref`、`allowed_domains`、`tenant_policy`。
- 外部账号绑定包含 `provider_id`、`external_subject`、`user_id`、`tenant_id`、`last_login_at`。
- `ai_api_key` 和 `ai_public_link` 用于后续 OpenAPI、外链分享和客户系统集成，必须绑定租户、应用、权限范围、QPS/用量限制、过期时间和审计记录。
- connector credential 必须包含 `scope_type`、`scope_id`、`auth_type`、`secret_ref`、`expires_at`、`scopes`。
- plugin installation 必须包含 `tenant_id`、`plugin_id`、`version`、`enabled`、`permission_grants`、`installed_by`。
- trigger 必须包含 `tenant_id`、`source_type`、`signature_secret_ref`、`route_target`、`idempotency_policy`。
- media job 必须包含 `tool_call_id`、`model_profile_id`、`asset_id`、`status`、`cost`、`latency_ms`、`policy_result`。

## 14. 部署和资源策略

### 14.1 POC 最小部署

为了节省 Docker 资源，POC 建议：

```text
backend            Rust API
admin              Next.js
postgres           主数据库，保存元数据、权限、chunk 文本、trace、业务配置
milvus             向量数据库，POC 用 Standalone，生产可升级 Cluster
parser-worker      Python sidecar，按需启动
object storage     本地文件或 MinIO 二选一
model provider     外部 API 或一个内网 OpenAI-compatible endpoint
embedding provider 外部 API 或一个内网 embedding endpoint
rerank provider    外部 API 或一个内网 rerank endpoint，可后置
```

RabbitMQ 当前 backend 已经有依赖。如果 POC 只跑单机，可以先用数据库任务表 + scheduler；任务量上来后再启 RabbitMQ。

### 14.2 不建议 POC 默认启用

- 完整 Dify 服务栈。
- 完整 FastGPT 服务栈。
- Elasticsearch。
- Kubernetes。
- 常驻 GPU parser。
- 多个本地大模型服务同时常驻。

这些组件不是不能用，而是不适合作为第一版必需依赖。Milvus 作为默认向量库保留，但 POC 使用 Standalone 或轻量部署，不默认启用 Milvus Cluster、Kubernetes 和复杂多副本运维。基座先把控制面、权限面、RAG/Agent 运行面打通，后续通过 adapter 替换底层实现。

### 14.3 生产部署

生产可以分三种：

1. Shared SaaS: 多租户共享服务，适合小客户和标准产品。
2. Dedicated Tenant: 单客户独立数据库或独立 namespace，适合中大型客户。
3. Private Deployment: 客户私有化部署，适合数据强监管场景。

每种部署都应支持：

- 配置导入导出。
- 租户级密钥。
- 模型供应商切换。
- Milvus collection、index、partition 策略迁移和容量规划。
- 租户级模型路由和内网模型 endpoint。
- parser worker 横向扩展。
- trace 和 eval 数据留存策略。

### 14.4 内网模型部署

内网部署不是后续补丁，而是模型适配层的核心目标。Novex 应支持三种模型接入方式：

1. Public API: OpenAI、DashScope、DeepSeek 等公有云模型。
2. Private OpenAI-compatible: 客户内网通过 vLLM、Ollama、TGI、Xinference、llama.cpp 或自研网关暴露统一接口。
3. Native Adapter: 对 embedding、rerank、VLM 等协议差异较大的模型，使用 `model-runtime` 或专用 adapter 归一化请求和返回。

内网模型配置必须支持：

- LLM: Qwen、DeepSeek、Gemma、Llama 等开源或私有部署模型。
- Embedding: bge、gte、qwen embedding、jina embedding 等。
- Rerank: bge-reranker、jina reranker、gte reranker 等。
- VLM: Qwen-VL、Gemma/Gemma-compatible vision、其他文档图像理解模型。

部署建议：

```text
Novex backend
  -> novex-model route
  -> model gateway / adapter
  -> public API 或 private model endpoint

Private model endpoint
  vLLM / Ollama / TGI / Xinference / llama.cpp / custom OpenAI-compatible proxy
```

私有化客户可以选择只部署 Novex + PostgreSQL + parser-worker，然后把模型 endpoint 指向客户已有推理平台。Novex 不应该强制内置模型推理服务；它应该管理配置、权限、路由、观测、评测和审计。

## 15. POC 交付范围

### 15.1 POC 目标

用最小系统证明 Novex 可以作为 AI Agent 基座，而不是只证明可以调一次 LLM。

POC 的原则是：基建骨架完整，业务功能切片收窄，演示入口前台化。也就是说，M0 必须把租户、权限、模型、Run、Trace、Job、Provider、Tool Policy、Eval Dataset 这些基础边界搭好；M1/M2 只选择员工培训系统这一个场景把闭环跑通；客户验收必须从 `apps/training-web` 进入，而不是从 `admin` 后台进入。

基建骨架必须包含：

- 租户和资源 ACL。
- AI 权限码和菜单。
- 模型供应商、模型部署、模型 profile、模型路由。
- Run、Trace、Cost、Usage、Feedback。
- Job 状态机和 worker 边界。
- Tool registry、risk policy、approval、audit。
- Identity provider、external account binding、secret 管理。
- Connector registry、credential binding、sync job。
- Plugin manifest、installation、permission grants。
- Trigger registry、webhook event、idempotency。
- Media job、media asset。
- Skill manifest 和版本。
- Eval dataset、eval case、eval run。

POC 业务切片必须证明：

- RBAC 能限制 AI 资源。
- 模型配置能在外部 API 和内网 OpenAI-compatible endpoint 之间切换。
- GitHub 登录能作为身份提供商接入，但不自动授予 GitHub repo 权限。
- GitHub connector 能独立完成 repo search/read，并有单独凭据绑定和审计。
- 图片生成工具能异步生成媒体资产，并记录模型、成本、trace 和权限结果。
- 文件能上传、解析、切片、检索、回答并引用。
- Skill 能配置、启用、执行和评测。
- Tool 能注册、授权、调用、审计。
- Agent 能完成 ReAct 流程，并把关键步骤写入 Run Event。
- Eval 能跑出报告。
- 客户模板能快速生成一个业务应用。
- Admin 能配置和治理应用，但 C 端前台能独立完成模型对话、知识库问答、测验和学习流程。

### 15.2 POC 场景：AI 员工培训系统

角色：

- 平台管理员。
- 客户管理员。
- HR / 培训管理员。
- 讲师。
- 员工。

功能：

Admin / 控制面功能：

- 上传培训资料。
- 自动解析并建立知识库。
- 配置培训应用入口、品牌、可见角色和默认知识库。
- 配置模型路由、embedding、rerank、出题 skill 和通知工具。
- HR / 培训管理员查看学习记录、薄弱知识点和评测报告。
- 管理员查看模型健康、RAG trace、Agent run、工具审计和评测报告。

C 端 / `apps/training-web` 功能：

- 员工进入培训工作台，看到待学习任务、资料入口、进度和通知状态。
- 员工围绕培训资料提问，答案带引用和反馈按钮。
- 员工基于资料生成或领取测验。
- 员工完成答题，看到解析、错题和相关资料引用。
- 员工查看自己的学习记录和薄弱知识点。
- 飞书通知学习任务，并能回到前台继续学习。

`apps/chat-web` 和 `apps/agent-workspace` 可以作为后续模板，但 POC 主演示必须是 `training-web`。`chat-web` 用来证明通用模型对话和知识库问答模板；`agent-workspace` 用来证明工具调用、Workflow/Run 执行视图和人工确认。

POC 不做：

- 完整 LMS。
- 复杂课程编排。
- 可视化 Workflow Builder / 低代码流程画布。
- 在 admin 内完成主要业务演示。
- 支付和合同。
- 多语言内容生产流水线。
- 自动改客户业务系统。

### 15.3 POC 验收标准

1. 两个租户的数据和知识库互不可见。
2. 一个租户内不同角色看到不同菜单和知识库。
3. 上传 PDF 或 Office 文档后，系统能完成解析、chunk、embedding。
4. 用户提问后，答案带至少一个可回溯引用。
5. 一个 skill 可以基于知识库生成 5 道培训题。
6. 一个工具调用可以发送飞书通知，且有权限和审计记录。
7. 一次 Agent run 可以在 trace 和 Run Event 中看到 intent、检索、工具调用、状态变化和最终输出。
8. 至少 20 条评测用例可以自动运行并生成报告。
9. POC 可以在本地低资源 docker-compose 或半本地模式运行。
10. 同一套 RAG 链路可以切换外部 embedding/rerank 和内网 embedding/rerank endpoint，且 trace 中能看到模型路由结果。
11. `apps/training-web` 能作为客户演示入口独立运行，员工不需要进入 admin 即可完成问答、测验和学习记录。
12. Admin 对同一套培训应用只承担配置、治理、审计和评测职责。

## 16. 里程碑

### M0: Foundation Skeleton

目标：把现有 RBAC 模板稳定为控制平面，并建立完整 AI 基建骨架。M0 不要求所有能力都可用，但所有核心边界、crate、表设计和配置入口必须成型，避免后续 RAG、Agent、Tools、MCP、Memory、Eval 各自重复造基础设施。

交付：

- Rust workspace 和 `crates/` 目录骨架，核心 AI 能力默认 Rust。
- `novex-ai-core`、`novex-model`、`novex-rag`、`novex-agent`、`novex-tools`、`novex-connectors`、`novex-mcp`、`novex-plugin`、`novex-trigger`、`novex-memory`、`novex-eval`、`novex-trace` crate 骨架。
- `services/parser-worker/` Python 目录骨架，限定为解析和 ML sidecar。
- `services/model-runtime/` 可选目录骨架，限定为内网模型和 ML adapter。
- `apps/` Next.js + TypeScript 目录骨架。
- `templates/` 客户模板目录骨架。
- 权限码规范。
- 菜单和角色初始化规范。
- 控制面租户、租户用户、租户角色、资源 ACL 设计。
- 资源权限、成员组、权限继承、配额、用量计量和限流策略设计。
- 身份提供商、外部账号绑定、OAuth state、secret 存储设计。
- 模型 provider、deployment、profile、credential、route、health、usage 设计。
- Milvus collection、index、partition、dimension、metric 管理设计。
- Run Graph、Run Step、Run Event、Pause、Trace、Cost、Usage、Feedback 通用结构。
- Job 状态机和 worker contract。
- Tool risk policy、approval、audit 基础结构。
- Connector credential、plugin manifest、trigger event 基础结构。
- API key、public link、外部集成入口的权限和限流基础结构。
- Media job、media asset 基础结构。
- Eval dataset、case、run 基础结构。
- 审计日志扩展方案。
- AI 菜单占位。
- 架构守则检查清单，覆盖模块归属、依赖方向、权限、secret、trace、eval 和 sidecar 边界。

### M1: 知识库 MVP

目标：完成最小 RAG 闭环，并让知识库问答先在 C 端可用。

交付：

- `novex-rag` crate 初版。
- `novex-model` route 在 RAG 中可用。
- `parser-worker` 初版。
- 知识库管理。
- 文档上传。
- parser job。
- chunk + embedding，embedding 模型从模型路由解析。
- Milvus 向量检索。
- rerank adapter 可配置，可先接外部 API 或内网 endpoint。
- 问答 API。
- 引用展示。
- RAG trace，记录 embedding、rerank、answer model 的路由结果。
- Admin 知识库控制面：数据集、文档、解析状态、检索测试。
- `apps/training-web` 问答页：员工基于培训资料提问、查看引用、提交反馈。
- `apps/chat-web` 最小知识库问答模板：支持会话、引用和模型路由展示。

### M2: Skills / Tools / Connectors / Plugins / MCP

目标：把能力从硬编码变成可配置。

交付：

- skill manifest。
- 模型选择策略可绑定到 skill。
- tool registry。
- tool call audit。
- connector registry 和凭据绑定。
- GitHub identity provider POC。
- GitHub connector POC。
- media/image tool POC。
- plugin manifest 和 installation POC。
- trigger registry 和 webhook POC。
- MCP server registry。
- 飞书消息工具 POC。
- Admin 能配置 skill、tool、connector、plugin、trigger 和权限。
- `apps/training-web` 能调用出题 skill 和飞书通知工具，但不暴露底层密钥和治理字段。
- C 端工具调用结果必须有用户可理解状态，同时在 Admin 可审计。

### M3: Agent Runtime

目标：完成 Codex-like 的工具使用循环，并跑通底层 Run Graph 状态机和 C 端执行视图。

交付：

- intent router。
- ReAct loop。
- context builder。
- tool selector。
- approval policy。
- run graph / run step / run event。
- pause / resume / cancel。
- human input 和 approval 的统一 pause reason。
- 断线重连后的 event snapshot。
- run trace。
- task budget。
- Admin Agent 控制面：策略、工具权限、运行记录、trace 和审计。
- `apps/agent-workspace` 最小运行页：展示 run 状态、步骤、变量、工具调用、暂停恢复和最终结果。
- POC Workflow 只展示任务/Agent run 的执行过程，不提供拖拽式低代码编辑器。

### M4: Eval

目标：让系统质量可测。

交付：

- eval dataset。
- eval case。
- eval runner。
- RAG 指标。
- intent 指标。
- tool 指标。
- 回归报告。
- Admin 评测报告页。
- C 端反馈入口：问答有用/无用、引用问题、测验错题反馈，进入 eval dataset 或 feedback 表。

### M5: 客户交付模板

目标：能快速落地一个客户项目。

交付：

- LLM Chat 默认模板。
- Knowledge Base Chat 默认模板。
- Agent Workspace 默认模板。
- 培训系统前台模板。
- 客户初始化向导。
- 品牌配置。
- 默认角色和菜单。
- 默认 skill。
- 默认 connector、plugin、trigger 配置。
- 默认 eval set。
- 部署手册。
- `apps/*` 模板发布配置：入口 URL、品牌、导航、默认页面、允许访问角色。
- `templates/*` 必须包含前台页面清单和 smoke test 脚本。
- 模板 apply 入口必须能幂等写入租户、角色、菜单、前台配置快照、能力 registry、内置插件安装、默认 eval set 选择和客户包快照，并返回剩余操作员步骤。
- 模板 smoke runner 必须能按 manifest 中的 smoke checks 生成 dry-run 计划或执行检查，并记录 run/result 明细。

## 17. 客户交付方法

每个客户项目尽量走配置化交付：

```text
Customer Package
  tenant config
  branding
  roles
  menus
  model routes
  model credentials
  datasets
  prompts
  skills
  run graph policies
  tools
  connectors
  plugins
  triggers
  eval cases
  frontend template config
```

交付流程：

1. 创建租户。
2. 选择行业模板。
3. 初始化菜单和角色。
4. 配置品牌和前台入口。
5. 配置模型路由、embedding、rerank 和密钥。
6. 配置身份提供商、连接器、插件和触发器。
7. 导入知识库。
8. 启用 skills。
9. 跑初始化评测。
10. 交付试运行。
11. 收集反馈进入评测集。

客户定制优先级：

1. 配置。
2. 模板。
3. skill。
4. connector。
5. run graph policy / flow template。
6. 代码改动。

代码改动应该是最后手段。

## 18. 安全和合规

必须内建：

- 租户隔离。
- RBAC + 资源 ACL。
- 模型 endpoint allow-list 和 network zone 限制。
- 模型密钥加密存储、轮换和租户隔离。
- 内网模型调用不得回落到外网模型，除非租户显式授权。
- OAuth state、redirect URI、scope、token refresh 和解绑审计。
- GitHub 登录和 GitHub connector 授权隔离。
- 插件权限声明、安装审批、启用范围和版本回滚。
- webhook 签名校验、幂等和重放攻击防护。
- 媒体生成内容策略、资产权限和外链有效期控制。
- 工具调用审批。
- 密钥加密存储。
- 外部请求 allow-list。
- prompt injection 防护。
- 文件类型和大小限制。
- 解析沙箱。
- 审计日志。
- 数据删除和导出。
- 模型供应商数据策略说明。

高风险工具示例：

- 发飞书消息。
- 写 GitHub issue / PR。
- 调外部 HTTP。
- 执行命令。
- 删除文档。
- 修改客户业务数据。

这些工具默认需要显式授权、权限码和审计。

## 19. 技术选型建议

### 19.1 语言决策

Novex 的核心后端尽可能使用 Rust。Rust 不是只用于 API 层，而是基座核心能力的主语言。

Rust 负责：

- 高并发 API。
- 权限和策略执行。
- Agent runtime 状态机。
- tool gateway。
- MCP gateway。
- model registry、model gateway、model routing。
- scheduler。
- trace 写入。
- 成本和限流。
- RAG orchestration。
- eval runner。
- connector gateway。

Python 允许存在，但定位必须清楚：它是插件运行时、解析运行时和 ML 生态适配层，不是核心业务控制面。文件解析、OCR、MinerU、部分 rerank、本地 embedding、本地模型或客户临时插件可以用 Python sidecar。Python 组件应通过 HTTP、gRPC、queue job 或 MCP/tool schema 与 Rust 通信，避免直接侵入核心业务逻辑。模型配置、模型路由、密钥、权限、审计和用量统计仍由 Rust 控制面管理。

前端统一使用 Next.js + TypeScript。管理后台、默认应用模板、客户交付前台都使用同一套前端技术栈，减少交付分叉。

不建议在第一阶段引入第三种后端主语言。Node.js 可以服务 Next.js 前端，不承担核心后端 API；Go、Java、Python Web API 都不作为基座主路径。

### 19.2 推荐组合

POC：

- Backend: Rust / Axum / SQLx。
- AI crates: Rust workspace。
- Admin: Next.js / TypeScript。
- Customer apps: Next.js / TypeScript。
- DB: PostgreSQL。
- Vector DB: Milvus Standalone。
- Storage: local file 或 MinIO。
- Queue: 先用 DB job + scheduler，必要时 RabbitMQ。
- Parser: Python worker + MinerU + LibreOffice。
- Model: 外部 API 或一个内网 OpenAI-compatible endpoint。
- Embedding: 外部 API 或一个内网 embedding endpoint。
- Rerank: 外部 API、内网 rerank endpoint 或后置。
- Observability: trace table + structured logs，后续接 OpenTelemetry。

生产：

- Core services: Rust。
- Plugin/parser services: Python sidecar。
- Model adapter/runtime: 可选，优先接客户已有内网推理平台。
- Frontend apps: Next.js / TypeScript。
- Vector DB 使用 Milvus，按规模从 Standalone 升级到 Cluster。
- Queue 可升级 RabbitMQ / Kafka。
- Storage 用 S3 / OSS / MinIO。
- Parser worker 独立扩缩容。
- Eval runner 独立运行。

## 20. 主要风险

1. 把 Dify 当底层硬依赖会限制后续产品化和权限体系。
2. 只做 RAG 会无法覆盖 Codex-like 任务和工具执行任务。
3. 过早做多 Agent 会让 POC 复杂度失控。
4. MinerU 对复杂 PDF 很有价值，但资源消耗和部署复杂度要单独评估。
5. 没有评测体系会导致每次 prompt、chunk、model 调整都无法判断质量变化。
6. 客户定制如果直接改代码，会快速形成多分支维护成本。
7. 模型适配如果散落在 RAG、Agent、Eval 代码里，后续接 Qwen、DeepSeek、Gemma、私有 embedding、私有 rerank 会形成大量重复逻辑。
8. 内网模型和外网模型混用时，如果没有 network zone、fallback policy 和审计，容易出现数据越界调用。
9. 过早把可视化 Workflow Builder 当主线，会把 POC 拖进低代码产品复杂度；底层 Run Graph 必须先稳定，再决定是否做可视化 flow。

## 21. 第一阶段行动清单

1. 建立 monorepo 模块边界：Rust `crates/`、Python `services/parser-worker/`、可选 `services/model-runtime/`、Next.js `apps/`、`templates/`、`infra/`。
2. 配置 Rust workspace，把 `backend` 接入 workspace，但保持现有 RBAC 功能可运行。
3. 在现有 RBAC 模板中加入控制面租户、`tenant_id`、资源 ACL 和 AI 权限码规划。
4. 设计资源权限表、成员组、权限继承、配额、用量计量和限流策略。
5. 新建系统身份提供商配置：GitHub/OIDC 登录、外部账号绑定、OAuth state 和 secret 存储。
6. 新建 AI 菜单和最小路由：Models、Knowledge、Agent、Tools、Connectors、Plugins、Triggers、Evals、Traces、Templates。
7. 设计 `novex-ai-core` 通用 Run Graph、Run Step、Run Event、Pause、Trace、Policy、TenantContext、ResourceRef。
8. 设计并迁移模型核心表：provider、deployment、profile、credential、route、health、usage。
9. 实现 `novex-model` 初版：OpenAI-compatible adapter、模型能力配置、租户级模型路由、用量归一化。
10. 设计 connectors、plugins、triggers、media jobs 核心表和权限策略。
11. 设计 API key、public link 和外部集成入口的权限与限流基础结构。
12. 设计并迁移知识库核心表。
13. 实现 `novex-rag` 初版：chunk、embedding、hybrid retrieval、citation，并通过 `novex-model` 获取 embedding/rerank/answer model。
14. 实现 `services/parser-worker` 初版：PDF、Office 转 PDF、MinerU 解析。
15. 实现文件上传到解析任务的 job 状态机。
16. 接入 Milvus Standalone，并建立 `ai_vector_collection` 到 Milvus collection 的映射。
17. 实现 RAG ask API，必须带引用和模型路由 trace。
18. 实现 LLM Chat 和 Knowledge Base Chat 默认模板。
19. 实现 tool registry、GitHub connector、图片生成工具和一个飞书工具。
20. 实现 plugin manifest、trigger registry 和 webhook POC。
21. 实现 Agent Runtime 的最小 Run Graph 状态机和事件日志。
22. 实现 skill manifest 和培训出题 skill。
23. 实现最小 eval runner。
24. 以员工培训系统作为第一个客户模板。
25. 对 M0-M3 的每个模块做架构守则检查：不得绕过 `novex-model`、`novex-ai-core`、Tool Registry、RBAC/ACL、secret、trace/audit。

## 22. 结论

Novex 更适合做一个 Codex-like 的 AI Agent 基座，而不是简单套一层 Dify 或 FastGPT。企业知识场景需要 RAG，研发和自动化场景需要 Agentic Tool Use，客户交付需要 RBAC、模板、连接器、插件、触发器、评测和可观测性。Workflow 不应作为第一阶段核心产品形态，但底层 Run Graph / Agent Runtime 必须保留，用来承载状态机、事件日志、暂停恢复、审批、取消、重试、审计和回放；可视化 Flow Builder 只作为后续高级能力。模型适配必须成为独立基建模块，统一管理公有云模型、内网 OpenAI-compatible endpoint、Qwen、DeepSeek、Gemma、embedding 和 rerank 模型。GitHub 登录这类能力应归入身份提供商，GitHub repo/issue 操作归入 connector + tool，画图归入 media tool，Dify-like 外部能力归入 plugin system。当前 Novex 的 Rust RBAC 模板已经适合作为控制平面，下一步应先完成租户化、身份提供商、模型适配层、Run Graph 和完整 AI 基建骨架，再用知识库 MVP、工具治理、技能包和评测闭环作为 POC 验收切片。
