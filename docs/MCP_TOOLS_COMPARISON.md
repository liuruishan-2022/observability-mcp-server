# Rust 版 MCP Tools 与开源实现对比

## 范围

本文整理了本次会话中关于本仓库 Rust 版 MCP tools 与开源 MCP 实现的对比结果，覆盖两个领域：

- Atlassian：Jira + Confluence
- Doris：数据库查询与分析工具

本次对比采用以下规则：

- 以功能能力为主
- 忽略工具命名差异
- 将“tool 能力覆盖”与“实现深度差异”“非 tool 的 MCP 能力”分开描述

## 对比基线

### 本仓库 Rust 实现

- Atlassian tools： [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)、[src/searcher/atlassian.rs](../observability-mcp-tools/src/searcher/atlassian.rs)
- Doris tools： [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)、[src/searcher/doris.rs](../observability-mcp-tools/src/searcher/doris.rs)

### 开源实现基线

- Atlassian：[`sooperset/mcp-atlassian`](https://github.com/sooperset/mcp-atlassian)，工具参考来自 [`llms-full.txt`](https://mcp-atlassian.soomiles.com/llms-full.txt)
- Doris：开源 Python 实现 [`apache/doris-mcp-server`](https://github.com/apache/doris-mcp-server)

## 总结

| 领域 | 功能结论 | 主要缺口 | 本地额外能力 |
| --- | --- | --- | --- |
| Atlassian | 相对选定的开源基线，功能上已覆盖 | 未发现明确的 tool 级缺口 | `confluence_get_space_page_tree` |
| Doris | 忽略命名差异后，tool 功能上已覆盖 | Rust 尚未实现原生 ADBC 执行 | FE/BE 状态、Query Stats、Load/Routine Load、Table Metadata |

## Atlassian

### 对比结论

以开源 `mcp-atlassian` 的工具参考为基线：

- Rust 实现当前暴露了 73 个 Jira/Confluence MCP tools，定义见 [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)
- 开源基线文档中可对应到 72 个 Jira/Confluence 具体工具
- 从功能覆盖角度看，Rust 实现已经覆盖本次对话中选定的开源基线
- Rust 还额外提供了一个 Confluence 能力：`confluence_get_space_page_tree`

### 功能覆盖范围

Rust 版已覆盖开源基线中的这些功能类别：

- Jira issue 查询与增删改
- Jira 字段与字段选项
- Jira 评论与状态流转
- Jira 项目与版本管理
- Jira Agile board、sprint、sprint issue 操作
- Jira links、worklog、attachments、watchers
- Jira Service Desk
- Jira Forms / ProForma
- Jira 指标与开发信息
- Confluence 页面、评论、标签、用户、分析、附件

对应入口定义见 [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)，核心客户端实现见 [src/searcher/atlassian.rs](../observability-mcp-tools/src/searcher/atlassian.rs)。

### Tool 级结论

在忽略命名差异的前提下，本次没有识别出 Rust 版相对于选定开源基线缺失的 Atlassian tool 功能。

### 运行时验证说明

本次对话里只做了有限的联调验证：

- Jira：`jira_get_all_projects`、`jira_get_link_types`、`jira_search`
- Confluence：`confluence_search`

因此这里的结论是“能力覆盖对比”，不是所有 Atlassian tools 都做过完整 smoke test。

## Doris

### 对比结论

以开源 Python `apache/doris-mcp-server` 为基线：

- Python 基线暴露 25 个 Doris tools，定义见 [`doris_mcp_server/tools/tools_manager.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/tools/tools_manager.py)
- Rust 实现暴露 31 个 Doris tools，定义见 [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)

如果忽略命名差异，Rust 版已经覆盖了 Python 基线在本次对话中涉及的全部 Doris tool 功能。

### 命名不同但功能等价的项

以下几组在本次对比中按功能等价处理：

- Python `get_db_list` ~= Rust `doris_get_databases`
- Python `get_db_table_list` ~= Rust `doris_get_tables`

### 已覆盖的功能范围

Rust 版已经覆盖 Python Doris 基线中的这些功能：

- SQL 执行
- 表结构、表注释、列注释、索引
- Catalog / Database 发现
- 最近审计日志
- SQL Explain / SQL Profile
- 表数据大小
- 监控指标
- 内存统计
- 表基础信息
- 列分析
- 表存储分析
- 列级血缘
- 数据新鲜度监控
- 数据访问模式分析
- 数据流依赖分析
- 慢查询分析
- 资源增长分析
- ADBC 相关诊断与查询接口

### 主要实现深度差距

当前最明确的差距是 ADBC 的实现深度，不是 tool 是否存在：

- Python `exec_adbc_query` 走的是真正的 ADBC / Arrow Flight SQL 执行路径，见 [`doris_mcp_server/utils/adbc_query_tools.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/utils/adbc_query_tools.py)
- Rust `doris_exec_adbc_query` 目前只是兼容接口，实际仍回退到 MySQL 查询执行，见 [src/searcher/doris.rs](../observability-mcp-tools/src/searcher/doris.rs)

这意味着：

- Tool 能力表面上已具备
- Rust 尚未具备原生 ADBC 执行能力

### `get_memory_stats` 说明

`get_memory_stats` 不是 Rust 独有的短板。

- Python 版在 [`doris_mcp_server/utils/analysis_tools.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/utils/analysis_tools.py) 中也明确是 placeholder 风格实现
- Rust 版在 [src/searcher/doris.rs](../observability-mcp-tools/src/searcher/doris.rs) 中也明确说明当前实现与官方 placeholder 行为一致

因此，这一项目前不构成相对 Python 基线的明显功能缺失。

### Rust 版额外提供的 Doris tools

相对于 Python 基线，Rust 版还额外提供了这些 Doris 运维能力：

- `doris_get_fe_status`
- `doris_get_be_status`
- `doris_get_query_stats`
- `doris_get_routine_loads`
- `doris_get_load_jobs`
- `doris_get_table_metadata`

对应入口见 [src/mcp/tools.rs](../observability-mcp-tools/src/mcp/tools.rs)。

## 非 Tool 的 MCP 能力

以下内容没有计入本次“tool 功能覆盖”主结论，但如果目标是完整的 MCP server 对齐，它们仍然重要。

### Atlassian

本次没有对开源 Atlassian server 的非 tool 能力做完整对比。

### Doris

Python 基线还具备一些不属于 Doris tool 本身的 MCP/server 能力：

- MCP resources，见 [`doris_mcp_server/tools/resources_manager.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/tools/resources_manager.py)
- MCP prompts，见 [`doris_mcp_server/tools/prompts_manager.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/tools/prompts_manager.py)
- OAuth、token-bound DB 配置、多租户 HTTP server 等服务层能力，见 [`doris_mcp_server/main.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/main.py) 与 [`doris_mcp_server/utils/db.py`](https://github.com/apache/doris-mcp-server/blob/master/doris_mcp_server/utils/db.py)

这些不影响“tool 功能已覆盖”的判断，但如果目标是完整 server 级能力对齐，Rust 这边仍有后续工作。

## 最终结论

### Atlassian

- 从 tool 功能角度看，Rust 版 Jira/Confluence MCP tools 已覆盖选定开源基线
- 本次没有识别出缺失的 Atlassian tool 功能
- Rust 版额外提供了 `confluence_get_space_page_tree`

### Doris

- 从 tool 功能角度看，Rust 版 Doris MCP tools 在忽略命名差异后已覆盖 Python 基线
- 主要剩余差距不是 tool 是否存在，而是实现深度：Rust 还没有真正接入原生 ADBC / Arrow Flight SQL 执行
- Rust 版还额外提供了若干 Doris 运维类工具

## 建议的后续优先级

如果后续要继续补齐能力，建议顺序如下：

1. 为 Rust Doris 实现原生 ADBC / Arrow Flight SQL 执行
2. 明确 Doris 是否需要继续对齐非 tool MCP 能力：resources、prompts、auth、多租户 server 行为
3. 如果需要生产级信心，对 Atlassian 和 Doris 全量 tools 追加 smoke test，而不是只做能力对比
