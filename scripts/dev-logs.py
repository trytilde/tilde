#!/usr/bin/env python3
"""Local ClickHouse logs setup. Defaults live in Compose; overrides use the existing .env."""
import os
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parent.parent


def configure(env):
    env = dict(env)
    env['TILDE_LOCAL_LOGS'] = '0'
    if env.get('LOGS_CLICKHOUSE_URL', '').strip() or env.get('DEV_LOGS_ENABLED', '1') == '0':
        return env
    port = int(env.get('LOGS_CLICKHOUSE_PORT', '18123'))
    if not 1 <= port <= 65535:
        raise ValueError('LOGS_CLICKHOUSE_PORT must be a valid port')
    env.update(TILDE_LOCAL_LOGS='1', LOGS_CLICKHOUSE_URL=f'http://127.0.0.1:{port}',
               LOGS_CLICKHOUSE_CONTAINER_URL='http://logs-clickhouse:8123')
    for key, value in [('LOGS_CLICKHOUSE_DATABASE', 'tilde_logs'), ('LOGS_CLICKHOUSE_USER', 'tilde'), ('LOGS_CLICKHOUSE_PASSWORD', 'tilde-logs-dev')]:
        if not env.get(key):
            env[key] = value
    return env


def start(env):
    if env.get('TILDE_LOCAL_LOGS') == '1':
        subprocess.run(['docker', 'compose', '--profile', 'logs', 'up', '-d', '--wait', 'logs-clickhouse'], cwd=ROOT, env=env, check=True)
        print('Local log storage: ClickHouse is ready')


if __name__ == '__main__':
    start(dict(os.environ))
