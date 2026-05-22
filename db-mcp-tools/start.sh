#!/usr/bin/env bash
set -euo pipefail

export HOST='127.0.0.1'
export PORT='3306'
export DATABASE='inter_gsms'
export USERNAME='liuxu'
export PASSWORD='liuxu@110'

cargo run -p db-mcp-tools -- -c ./db-mcp-tools/db-tools.yaml
