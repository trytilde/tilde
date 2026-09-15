#!/usr/bin/env python3
"""Start the local Compose stack using development defaults and Task's .env values."""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = ('LANGFUSE_BASE_URL', 'LANGFUSE_PUBLIC_KEY', 'LANGFUSE_SECRET_KEY')


def configure(env):
    env = dict(env)
    supplied = [bool(env.get(k, '').strip()) for k in REQUIRED]
    if any(supplied):
        # User configuration, including incomplete configuration, is never replaced.
        env['TILDE_LOCAL_LANGFUSE'] = '0'
        return env
    if env.get('DEV_LANGFUSE_ENABLED', '1') == '0':
        env['TILDE_LOCAL_LANGFUSE'] = '0'
        return env
    # These development keys match Compose's headless project initialization.
    env['LANGFUSE_PUBLIC_KEY'] = env.get('DEV_LANGFUSE_PUBLIC_KEY') or 'pk-lf-tilde-local'
    env['LANGFUSE_SECRET_KEY'] = env.get('DEV_LANGFUSE_SECRET_KEY') or 'sk-lf-tilde-local'
    port = int(env.get('LANGFUSE_PORT', '3003'))
    if not 1 <= port <= 65535:
        raise ValueError('LANGFUSE_PORT must be a valid port')
    address = env.get('TS_ADDR') or '127.0.0.1'
    # Host clients must use the same interface that Compose publishes.
    env.update(LANGFUSE_BASE_URL=f'http://{address}:{port}',
               LANGFUSE_PUBLIC_URL=env.get('LANGFUSE_PUBLIC_URL') or f'http://{address}:{port}',
               LANGFUSE_DEV_ADDRESS=address, LANGFUSE_CONTAINER_BASE_URL='http://langfuse-web:3000', TILDE_LOCAL_LANGFUSE='1')
    return env


def start(env):
    if env.get('TILDE_LOCAL_LANGFUSE') != '1':
        return
    command = ['docker', 'compose', '--profile', 'langfuse']
    subprocess.run([*command, 'up', '-d', '--wait', 'langfuse-postgres', 'langfuse-clickhouse', 'langfuse-redis', 'minio'], cwd=ROOT, env=env, check=True)
    subprocess.run([*command, 'run', '--rm', 'langfuse-bucket'], cwd=ROOT, env=env, check=True)
    subprocess.run([*command, 'up', '-d', 'langfuse-web', 'langfuse-worker'], cwd=ROOT, env=env, check=True)
    deadline = time.monotonic() + 240
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(env['LANGFUSE_BASE_URL'] + '/api/public/health', timeout=5) as response:
                if response.status == 200:
                    auth = base64.b64encode((env['LANGFUSE_PUBLIC_KEY'] + ':' + env['LANGFUSE_SECRET_KEY']).encode()).decode()
                    request = urllib.request.Request(env['LANGFUSE_BASE_URL'] + '/api/public/projects', headers={'Authorization': 'Basic ' + auth})
                    with urllib.request.urlopen(request, timeout=5) as projects:
                        if not json.load(projects).get('data'):
                            raise RuntimeError('Langfuse project has not initialized')
                    print('Local Langfuse: ' + env['LANGFUSE_PUBLIC_URL'])
                    print('Login: developer@tilde.local (password: LANGFUSE_DEV_LOGIN_PASSWORD in .env, or the Compose default)')
                    return
        except Exception:
            time.sleep(2)
    raise RuntimeError('Langfuse did not become ready within 240 seconds; inspect docker compose logs langfuse-web langfuse-worker')


if __name__ == '__main__':
    try:
        start(dict(os.environ))
    except KeyboardInterrupt:
        sys.exit(130)
    except (ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        # Subprocess command contains service names only, never credentials.
        sys.exit(str(error))
