#!/usr/bin/env python3
"""Extract this runtime's chat and development credentials from the original Tilde SOPS document.

The encrypted document retains its original KMS recipient and top-level/test
layout. Never source generated dotenv files as shell code: PEMs and tokens may
contain shell metacharacters. Task's dotenv parser loads them as data.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def selected(document):
    prefixes = ("tool_managed_provider_slack_", "tool_managed_provider_github_",
                "tool_managed_provider_whatsapp_", "e2e_chatkit_", "e2e_linq_")
    names = {"linq_api_token", "e2e_mcp_linq_api_token", "hookdeck_api_key", "e2e_mcp_whatsapp_recipient"}
    return {k.upper(): v for k, v in document.items()
            if isinstance(v, str) and (k.startswith(prefixes) or k in names)}


def render_env(values):
    # godotenv supports escaped double quotes/backslashes and literal multiline values.
    lines = ["# Generated from secrets.enc.yaml; DO NOT COMMIT or source in a shell."]
    for key, value in sorted(values.items()):
        if not re.fullmatch(r"[A-Z_][A-Z0-9_]*", key):
            raise ValueError("Invalid environment variable name")
        value = value.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$")
        lines.append(f'{key}="{value}"')
    return "\n".join(lines) + "\n"


def write_env(path, values):
    content = render_env(values)
    fd, tmp = tempfile.mkstemp(prefix=".load-secrets.", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as out:
            out.write(content)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def main():
    # stdin JSON also allows an existing local decrypted copy to be migrated
    # without requiring fresh AWS credentials. Normal operation always uses SOPS.
    if sys.argv[1:] == ["--stdin-json"]:
        document = json.load(sys.stdin)
    elif not sys.argv[1:]:
        result = subprocess.run(
            ["sops", "decrypt", "--output-type", "json", str(ROOT / "secrets.enc.yaml")],
            check=True, stdout=subprocess.PIPE)
        document = json.loads(result.stdout)
    else:
        raise ValueError("Usage: load-secrets.py [--stdin-json]")
    dev = selected(document)
    test = {**dev, **selected(document.get("test", {}))}
    # The tunnel credential belongs only to the interactive development launcher.
    for name in ["ngrok_authtoken", "openai_api_key"]:
        if isinstance(document.get(name), str):
            dev[name.upper()] = document[name]
    credential = document.get("test", {}).get("e2e_mcp_whatsapp_credential_json")
    if credential:
        for key, value in json.loads(credential).items():
            if isinstance(value, str):
                test[f"CHAT_TEST_WHATSAPP_{key.upper()}"] = value
    # Validate both outputs before replacing either working file.
    render_env(dev)
    render_env(test)
    write_env(ROOT / ".env.secrets", dev)
    write_env(ROOT / ".env.secrets.test", test)
    print("Wrote private .env.secrets and .env.secrets.test")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, subprocess.CalledProcessError):
        sys.exit("Secret loading failed; existing dotenv files were retained. Check SOPS/AWS access and document structure.")
