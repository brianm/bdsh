---
yatl_version: 1
title: Generate consensus view with diff highlighting
id: vyz6nsmn
created: 2026-01-18T21:41:40.087427Z
updated: 2026-01-18T21:42:03.945263Z
author: Brian McCallister
priority: high
tags:
- client-mode
- ui
blocked_by:
- 07v1fs60
---

The core value proposition: show what's common across hosts and highlight differences.

Algorithm:
1. Read out.log from each host
2. Find common lines (consensus)
3. Identify lines that differ between hosts
4. Display consensus with markers for divergence

Display format ideas:
- Show consensus lines normally
- For divergent sections, show inline diffs or expandable sections
- Color-code by host when showing differences
- Show summary: "5 hosts agree, 2 differ at line 42"

Use mitsuhiko/similar crate (mentioned in README) for diffing.

Consider:
- Streaming output (files still being written)
- Large output handling (pagination, truncation)
- Terminal width handling

---
# Log: 2026-01-18T21:41:40Z Brian McCallister

Created task.

---
# Log: 2026-01-18T21:42:03Z Brian McCallister

Added blocker: 07v1fs60
