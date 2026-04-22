FROM xwharbor.wxchina.com/cpaas/component/ubuntu:24.10

COPY ./target/release/observability-mcp-server /opt/observability-mcp-server
WORKDIR /opt
ENTRYPOINT ["/opt/observability-mcp-server"]
