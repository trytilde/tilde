#!/usr/bin/env python3
"""Refresh the vendored inference price sheet from LiteLLM's community-maintained table.

Source: https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json
(MIT). Only the providers Tilde routes and the per-token fields Tilde bills are kept, in
micro-dollars per million tokens, so the checked-in file stays small and diffable. The engine
reconciles it into `inference_prices` at startup; rows edited by hand keep source 'manual'.
"""
import json
import sys
import urllib.request
from pathlib import Path

URL = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
OUT = Path(__file__).resolve().parent.parent / "crates/tilde/src/inference/prices.json"
# Tilde provider id -> LiteLLM provider labels whose rows apply, and the key prefix to strip.
PROVIDERS = {
    "openai": (["openai"], ""),
    "anthropic": (["anthropic"], ""),
    "azure_openai": (["azure"], "azure/"),
    "azure_ai": (["azure_ai"], "azure_ai/"),
    "google_ai": (["gemini"], "gemini/"),
    "bedrock": (["bedrock", "bedrock_converse", "bedrock_mantle"], "bedrock/"),
    "baseten": (["baseten"], "baseten/"),
    "cerebras": (["cerebras"], "cerebras/"),
    "vercel_ai_gateway": (["vercel_ai_gateway"], "vercel_ai_gateway/"),
    "cohere": (["cohere", "cohere_chat"], "cohere/"),
    "voyage": (["voyage"], "voyage/"),
}


def per_million_micros(price_per_token):
    if price_per_token is None:
        return None
    return round(float(price_per_token) * 1_000_000 * 1_000_000)


def main():
    source = sys.argv[1] if len(sys.argv) > 1 else URL
    if source.startswith("http"):
        with urllib.request.urlopen(source, timeout=60) as response:
            table = json.load(response)
    else:
        table = json.loads(Path(source).read_text())
    rows = []
    seen = set()
    for key, entry in table.items():
        # Any billable unit qualifies: tokens, images, characters or audio seconds.
        if not isinstance(entry, dict) or not any(
            entry.get(k) is not None
            for k in ("input_cost_per_token", "output_cost_per_image", "input_cost_per_character", "input_cost_per_second")
        ):
            continue
        label = entry.get("litellm_provider")
        for provider, (labels, prefix) in PROVIDERS.items():
            if label not in labels:
                continue
            model = key[len(prefix):] if prefix and key.startswith(prefix) else key
            if "/" in model and provider != "bedrock":
                continue
            # LiteLLM lists some models under two labels (e.g. bedrock and bedrock_converse).
            if (provider, model) in seen:
                continue
            seen.add((provider, model))
            rows.append({
                "provider_id": provider,
                "model": model,
                "input_per_m_micros": per_million_micros(entry.get("input_cost_per_token")),
                "output_per_m_micros": per_million_micros(entry.get("output_cost_per_token")),
                "cached_input_per_m_micros": per_million_micros(entry.get("cache_read_input_token_cost")),
                "cache_write_per_m_micros": per_million_micros(entry.get("cache_creation_input_token_cost")),
                "image_micros": None if entry.get("output_cost_per_image") is None else round(float(entry["output_cost_per_image"]) * 1_000_000),
                "character_per_m_micros": per_million_micros(entry.get("input_cost_per_character")),
                "second_micros": None if entry.get("input_cost_per_second") is None else round(float(entry["input_cost_per_second"]) * 1_000_000),
            })
    rows.sort(key=lambda r: (r["provider_id"], r["model"]))
    OUT.write_text(json.dumps(rows, indent=0, separators=(",", ":")) + "\n")
    by_provider = {}
    for row in rows:
        by_provider[row["provider_id"]] = by_provider.get(row["provider_id"], 0) + 1
    print(f"wrote {len(rows)} prices to {OUT.relative_to(OUT.parents[4])}: {by_provider}")


if __name__ == "__main__":
    main()
