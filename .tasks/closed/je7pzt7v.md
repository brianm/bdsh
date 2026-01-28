---
yatl_version: 1
title: Add config file with host tagging
id: je7pzt7v
created: 2026-01-27T02:44:19.277383Z
updated: 2026-01-28T04:54:16.672690Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
---

Add ~/.config/bdsh/config (or XDG_CONFIG_HOME/bdsh/config) for defining hosts with tags.

Format (one host per line):
```
freki.home       :bsd :dev
badb.home        :bsd :dev  
hati.brianm.dev  :bsd :utility
```

Usage:
- `bdsh :bsd -- cmd` runs on all hosts tagged :bsd
- `bdsh :dev -- cmd` runs on all hosts tagged :dev
- Tags are additive: `bdsh :bsd:dev -- cmd` runs on hosts with BOTH tags

Config file can be executable (same pattern as @executable) - if executable, run it and parse output.

Implementation:
1. Add config loading to hosts.rs
2. Detect :tag syntax in parse_host_spec
3. Filter hosts by matching tags

---
# Log: 2026-01-27T02:44:19Z Brian McCallister

Created task.

---
# Log: 2026-01-27T02:44:42Z Brian McCallister

Started working.

---
# Log: 2026-01-27T02:45:17Z Brian McCallister

Stopped working.

---
# Log: 2026-01-27T02:45:17Z Brian McCallister

Partial implementation: created config.rs with TaggedHost parsing, tag filtering (AND logic), and tests. Not yet wired into hosts.rs.

---
# Log: 2026-01-27T02:45:35Z Brian McCallister

Design note: comma for OR logic. ':bsd,:linux' means hosts with bsd OR linux tags.

---
# Log: 2026-01-28T04:54:16Z Brian McCallister

Closed: Already implemented in hosts.rs - config file loading, tag parsing, and AND/OR filtering all working
