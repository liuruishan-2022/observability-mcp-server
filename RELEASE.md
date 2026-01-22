# 构建和发布

## 自动构建

项目使用 GitHub Actions 自动构建多平台二进制文件。

### 触发构建

#### CI 构建
每次 push 到 `main` 或 `develop` 分支，或创建 Pull Request 时，会自动运行 CI：
- 代码格式检查
- Clippy 静态分析
- 单元测试
- 多平台构建检查

#### Release 构建
当推送以 `v` 开头的 tag 时（如 `v1.0.0`），会自动��
1. 构建所有目标平台的二进制文件
2. 创建 GitHub Release
3. 上传所有构建产物到 Release

### 支持的平台

| 平台 | 架构 | 文件名 |
|------|------|--------|
| Linux | x86_64 (AMD64) | `observability-mcp-server-linux-amd64` |
| Linux | ARM64 (AArch64) | `observability-mcp-server-linux-arm64` |
| macOS | x86_64 (Intel) | `observability-mcp-server-darwin-amd64` |
| macOS | ARM64 (Apple Silicon) | `observability-mcp-server-darwin-arm64` |
| Windows | x86_64 (AMD64) | `observability-mcp-server-windows-amd64.exe` |

### 发布新版本

1. 更新 `Cargo.toml` 中的版本号
2. 提交更改：
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to x.y.z"
   ```
3. 创建并推送 tag：
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
4. GitHub Actions 会自动构建并创建 Release

### 手动触发构建

你可以在 GitHub Actions 页面手动选择 `Build and Release` workflow 并触发它。

## 本地构建

### 前提条件

- Rust 工具链 (1.81+)
- Make (可选)

### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/yourusername/observability-mcp-server.git
cd observability-mcp-server

# 构建_release 版本
cargo build --release

# 二进制文件位置
# Linux/macOS: target/release/observability-mcp-server
# Windows: target/release/observability-mcp-server.exe
```

### 交叉编译

#### Linux ARM64
```bash
rustup target add aarch64-unknown-linux-gnu
sudo apt-get install gcc-aarch64-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

#### macOS
```bash
# Intel (x86_64)
cargo build --release --target x86_64-apple-darwin

# Apple Silicon (ARM64)
cargo build --release --target aarch64-apple-darwin
```

#### Windows
```bash
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

### 运行

```bash
# 配置环境变量
cp .env.example .env
# 编辑 .env 文件，设置 PROMETHEUS_ROOT 和 LOKI_ROOT

# 运行服务
./target/release/observability-mcp-server
```

## Docker 构建 (可选)

如果需要使用 Docker，可以创建 `Dockerfile`：

```dockerfile
FROM rust:1.81 as builder
WORKDIR /usr/src/observability-mcp-server
COPY . .
RUN cargo install --path .

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/observability-mcp-server /usr/local/bin/
ENTRYPOINT ["observability-mcp-server"]
```

构建和运行：
```bash
docker build -t observability-mcp-server .
docker run -e PROMETHEUS_ROOT=http://prometheus:9090 -e LOKI_ROOT=http://loki:3100 observability-mcp-server
```
