# Observability MCP Server - Prometheus 文档工具使用说明

## 概述

本 MCP 服务器现在包含了三个用于查询 Prometheus 官方文档的工具：

1. **docs_list** - 列出所有可用的文档文件
2. **docs_read** - 读取指定文档文件的内容
3. **docs_search** - 在文档中搜索关键词

## 初始化文档加载器

文档加载器需要从包含 Prometheus 文档的目录初始化。默认情况下，文档应该放在项目的 `docs/` 目录下。

### 准备 Prometheus 文档

```bash
# 克隆 Prometheus 官方文档仓库
git clone https://github.com/prometheus/docs.git /tmp/prometheus-docs

# 将文档复制到项目目录
mkdir -p observability-mcp-server/docs
cp -r /tmp/prometheus-docs/content/*.md observability-mcp-server/docs/
```

### 在代码中初始化

在 `main.rs` 中，你需要初始化文档加载器：

```rust
use crate::docs::{create_shared_docs_loader, init_docs_loader};

#[tokio::main]
async fn main() -> Result<()> {
    // ... 其他初始化代码 ...

    // 创建共享的文档加载器
    let docs_loader = create_shared_docs_loader();

    // 初始化文档加载器（从 docs/ 目录）
    let docs_dir = "./docs";
    if let Err(e) = init_docs_loader(&docs_loader, docs_dir).await {
        eprintln!("Failed to initialize docs loader: {}", e);
        eprintln!("Continuing without docs support...");
    } else {
        info!("Documentation loaded successfully from: {}", docs_dir);
    }

    // 创建 Tools 实例并注入文档加载器
    let tools = Tools::new().with_docs_loader(docs_loader);

    // ... 其他代码 ...
}
```

## MCP 工具使用示例

### 1. docs_list - 列出所有文档文件

**请求：**
```json
{
  "name": "docs_list",
  "arguments": {}
}
```

**响应：**
```json
{
  "files": [
    "configuring/prometheus.md",
    "getting_started.md",
    "querying/basics.md",
    "storage.md",
    "visualization/grafana.md"
  ],
  "count": 5
}
```

### 2. docs_read - 读取文档内容

**请求：**
```json
{
  "name": "docs_read",
  "arguments": {
    "file": "getting_started.md"
  }
}
```

**响应：**
```json
{
  "file": "getting_started.md",
  "content": "# Getting Started\n\nPrometheus is an open-source monitoring..."
}
```

### 3. docs_search - 搜索文档

**请求：**
```json
{
  "name": "docs_search",
  "arguments": {
    "query": "recording rules",
    "limit": 10
  }
}
```

**响应：**
```json
{
  "query": "recording rules",
  "matching_files": [
    "configuring/prometheus.md",
    "querying/basics.md",
    "rules.md"
  ],
  "count": 3
}
```

## 实现细节

### 文档分块

为了优化搜索性能，文档被分成多个块：
- 块大小：4096 字符
- 重叠大小：512 字符

这确保即使文档很长，搜索也能快速定位相关部分。

### 搜索算法

当前实现使用简单的关键词匹配：
1. 将查询和内容都转换为小写
2. 统计每个块中匹配的出现次数
3. 按匹配次数排序
4. 返回前 N 个最匹配的文件名

### 未来改进

可能的改进方向：
1. 使用更高级的全文搜索引擎（如 Tantivy）
2. 支持模糊匹配
3. 添加相关度评分
4. 支持多个关键词的组合查询
5. 缓存搜索结果

## 环境变量

无需额外的环境变量。文档路径在代码中硬编码或通过配置传递。

## 故障排查

### 问题：文档加载失败

**错误信息：** `Error: Docs loader not initialized`

**解决方案：**
1. 确保 `docs/` 目录存在
2. 确保目录中包含 `.md` 文件
3. 检查文件权限

### 问题：搜索没有结果

**解决方案：**
1. 尝试使用更通用的关键词
2. 确认关键词拼写正确
3. 使用 `docs_list` 查看可用的文档文件

### 问题：读取文件失败

**错误信息：** `Error: File not found: xxx.md`

**解决方案：**
1. 使用 `docs_list` 获取正确的文件名
2. 文件名区分大小写
3. 确保文件名包含 `.md` 扩展名
