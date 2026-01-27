---
yatl_version: 1
title: Parse inline comma-separated hosts
id: 31n7nrrq
created: 2026-01-26T04:57:28.187572Z
updated: 2026-01-27T02:08:42.873942Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- 3132nkry
---

Handle the simple case: `bdsh host1,host2,host3 -- cmd`

Create `src/hosts.rs` module with:
- `Host` struct (hostname + optional metadata columns)
- `parse_inline(spec: &str) -> Vec<Host>` - split on comma

If spec doesn't start with `@`, treat as inline comma-separated list.

---
# Log: 2026-01-26T04:57:28Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:57:35Z Brian McCallister

Added blocker: 3132nkry

---
# Log: 2026-01-27T02:08:42Z Brian McCallister

Closed: Implemented in spike - splits host_spec on comma
