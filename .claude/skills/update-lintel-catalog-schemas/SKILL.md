---
name: update-lintel-catalog-schemas
description: >
  Regenerate JSON Schemas for lintel config types into ../catalog/schemas/lintel/.
  Use when the user asks to "update schemas", "regenerate schemas",
  "update lintel catalog schemas", or "run update-lintel-catalog-schemas".
allowed-tools:
  - Bash(./scripts/update-lintel-catalog-schemas)
---

# Update Lintel Catalog Schemas

Run `./scripts/update-lintel-catalog-schemas` to regenerate the JSON Schemas for all config types into `../catalog/schemas/lintel/`.

This produces three schemas:

- `lintel-toml.json` — from `lintel_config::schema()`
- `lintel-catalog-toml.json` — from `lintel_catalog_builder::config::schema()`
- `catalog.json` — from `schema_catalog::schema()`
