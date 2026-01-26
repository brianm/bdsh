---
yatl_version: 1
title: Parse YAML host data with json-pointer
id: hqxt0dwn
created: 2026-01-26T04:58:40.525416Z
updated: 2026-01-26T04:58:46.039920Z
author: Brian McCallister
priority: medium
tags:
- cli
- hosts
blocked_by:
- 3d2ymbad
---

Parse YAML host data using same approach as JSON:

- YAML arrays work like JSON arrays
- Use `--host-ptr` json-pointer (works on YAML since it's a superset)

Add `serde_yaml` crate to Cargo.toml.

Can potentially share code with JSON parsing by deserializing to serde_json::Value.

---
# Log: 2026-01-26T04:58:40Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:58:46Z Brian McCallister

Added blocker: 3d2ymbad
