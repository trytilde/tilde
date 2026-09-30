#!/usr/bin/env python3
"""Local ClickHouse for log, span and metric history. The engine requires ClickHouse, so dev always
starts one unless ENGINE_CLICKHOUSE_URL names an existing server. Defaults live in Compose."""
import os
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parent.parent


def configure(env):
    env = dict(env)
    env['TILDE_LOCAL_CLICKHOUSE'] = '0'
    if env.get('ENGINE_CLICKHOUSE_URL', '').strip():
        return env
    port = int(env.get('ENGINE_CLICKHOUSE_PORT', '18123'))
    if not 1 <= port <= 65535:
        raise ValueError('ENGINE_CLICKHOUSE_PORT must be a valid port')
    env.update(TILDE_LOCAL_CLICKHOUSE='1', ENGINE_CLICKHOUSE_URL=f'http://127.0.0.1:{port}',
               ENGINE_CLICKHOUSE_CONTAINER_URL='http://logs-clickhouse:8123')
    # History is queued in these buckets; task dev creates them on local MinIO.
    for key, value in [('ENGINE_LOGS_S3_BUCKET', 'tilde-logs'), ('ENGINE_TRACES_S3_BUCKET', 'tilde-traces'), ('ENGINE_METRICS_S3_BUCKET', 'tilde-metrics'), ('ENGINE_CLICKHOUSE_DATABASE', 'tilde_logs'), ('ENGINE_CLICKHOUSE_USER', 'tilde'), ('ENGINE_CLICKHOUSE_PASSWORD', 'tilde-logs-dev')]:
        if not env.get(key):
            env[key] = value
    return env


def start(env):
    if env.get('TILDE_LOCAL_CLICKHOUSE') == '1':
        subprocess.run(['docker', 'compose', '--profile', 'logs', 'up', '-d', '--wait', 'logs-clickhouse'], cwd=ROOT, env=env, check=True)
        print('Local log and trace storage: ClickHouse is ready')


if __name__ == '__main__':
    start(dict(os.environ))
