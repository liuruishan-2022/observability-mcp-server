FROM xwharbor.wxchina.com/cpaas/component/ubuntu:24.10

RUN sed -i 's|http://archive.ubuntu.com/ubuntu|http://old-releases.ubuntu.com/ubuntu|g; s|http://security.ubuntu.com/ubuntu|http://old-releases.ubuntu.com/ubuntu|g' /etc/apt/sources.list /etc/apt/sources.list.d/*.list 2>/dev/null || true

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

COPY ./target/release/observability-mcp-server /opt/observability-mcp-server
WORKDIR /opt
ENTRYPOINT ["/opt/observability-mcp-server"]
