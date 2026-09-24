#!/usr/bin/env python3
"""Apply TS_ADDR to local dev services after Task loads dotenv, then run Task's services."""
import importlib.util
from pathlib import Path
import ipaddress
import os
from urllib.parse import urlsplit, urlunsplit

spec = importlib.util.spec_from_file_location("dev_langfuse", Path(__file__).with_name("dev-langfuse.py"))
langfuse = importlib.util.module_from_spec(spec)
spec.loader.exec_module(langfuse)
env = langfuse.configure(os.environ)
spec = importlib.util.spec_from_file_location("dev_logs", Path(__file__).with_name("dev-logs.py"))
logs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logs)
env = logs.configure(env)
address = env.get("TS_ADDR") or env.get("ADDRESS") or "127.0.0.1"
try:
    ip = ipaddress.IPv4Address(address)
except ValueError:
    raise SystemExit("TS_ADDR must be an IPv4 address")

local_hosts = {"localhost", "127.0.0.1", "::1"}
env.setdefault("ENGINE_RUNTIME_PUBLIC_URL", f"http://{address}:{env.get("API_PORT", "8080")}")
public_port = env.get("API_PORT", "8080") if env.get("ENGINE_WEB_ENABLED") == "false" else env.get("WEB_PORT", "5173")
env.setdefault("ENGINE_PUBLIC_URL", f"http://{address}:{public_port}")
if env.get("TS_ADDR"):
    env["ADDRESS"] = address

if not ip.is_loopback:
    env["ENGINE_ALLOW_NETWORK"] = "true"
    if env.get("WEB_HOST", "127.0.0.1") in local_hosts:
        env["WEB_HOST"] = address
    for key, port in [("ENGINE_LISTEN", env.get("API_PORT", "8080"))]:
        host, _, port = env.get(key, f"127.0.0.1:{port}").rpartition(":")
        if host.strip("[]") in local_hosts:
            env[key] = f"{address}:{port}"

    def relocate(value):
        url = urlsplit(value)
        if url.hostname not in local_hosts or url.username or url.password:
            return value
        authority = address + (f":{url.port}" if url.port else "")
        return urlunsplit(url._replace(netloc=authority))

    # Browser-facing services must agree on their origins, including OAuth and HMR.
    # Database addresses are independent of TS_ADDR.
    for key in [
        "ENGINE_PUBLIC_URL", "ENGINE_INGRESS_PUBLIC_URL", "ENGINE_RUNTIME_PUBLIC_URL",
        "ENGINE_CONNECTION_SETUP_PUBLIC_URL", "ENGINE_CONNECTION_UI_DEV_URL",
        "ENGINE_DEV_URL", "ENGINE_OIDC_ISSUER", "DEX_DEV_ISSUER",
        "DEX_DEV_WEB_CALLBACK", "DEX_DEV_API_CALLBACK",
        "ENGINE_S3_ENDPOINT", "ENGINE_S3_PUBLIC_ENDPOINT",
    ]:
        if env.get(key):
            env[key] = relocate(env[key])
    if env.get("ENGINE_WEB_ORIGINS"):
        origins = env["ENGINE_WEB_ORIGINS"].split(",")
        env["ENGINE_WEB_ORIGINS"] = ",".join(dict.fromkeys(origins + [relocate(origin.strip()) for origin in origins]))

# Exported values beat dotenv when the nested Task process loads the files again.
os.execvpe("task", ["task", "dev:run"], env)
