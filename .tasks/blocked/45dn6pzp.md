---
yatl_version: 1
title: Parse @file host source
id: 45dn6pzp
created: 2026-01-26T04:57:38.261091Z
updated: 2026-01-26T04:57:43.918768Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- 31n7nrrq
---

Handle `@./path` syntax where path is a regular file.

In `src/hosts.rs`:
- `parse_source(spec: &str) -> Vec<Host>` - if starts with @, read file
- Read file contents, pass to format detection

For now, just read as plain text with one host per line (format detection comes later).

---
# Log: 2026-01-26T04:57:38Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:57:43Z Brian McCallister

Added blocker: 31n7nrrq
