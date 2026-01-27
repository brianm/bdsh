---
yatl_version: 1
title: Execute @executable host source
id: 7a0a2txt
created: 2026-01-26T04:57:47.107266Z
updated: 2026-01-27T02:44:07.235609Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- 45dn6pzp
---

If `@./path` is executable, run it and use stdout as host source.

Check if file has execute permission, if so:
- Spawn process and capture stdout
- Pass output to format detection (same as file content)

This enables dynamic host lists from scripts, APIs, etc.

---
# Log: 2026-01-26T04:57:47Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:57:53Z Brian McCallister

Added blocker: 45dn6pzp

---
# Log: 2026-01-27T02:37:19Z Brian McCallister

Started working.

---
# Log: 2026-01-27T02:44:07Z Brian McCallister

Closed: Implemented executable host source. If @path has execute permission, runs it and parses stdout. Added 5 new tests for executable parsing.
