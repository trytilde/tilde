# Provider icon sources

Static assets referenced by built-in providers as `/provider-icons/<file>`, so the UI never
depends on a third-party icon CDN at runtime. Imported from trytilde/api, which took most of
them from the default SVG variants in [`glincker/thesvg`](https://github.com/glincker/thesvg).
Brands not present there use their own published assets: Browserbase
(`browserbase.com/favicon.svg`), E2B (its wordmark SVG), Parallel (`parallel.ai/icon.svg`),
Clay (the `clay.com` favicon) and Entire (`entire.com/favicon.svg`); the MCP logo is api's
`images/clients/mcp.svg`. GitLab, Jira, Datadog, Grafana, Prometheus, PagerDuty, New Relic,
Honeycomb and Terraform come from thesvg directly (`@thesvg/icons` 4.0.14, default variants;
Honeycomb is the mark-only `mono` variant in its brand colour, since its default is a wordmark).
