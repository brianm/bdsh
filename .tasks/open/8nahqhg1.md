---
yatl_version: 1
title: 'Bug: unwrap() calls can panic on edge cases'
id: 8nahqhg1
created: 2026-01-28T04:40:25.448731Z
updated: 2026-01-28T04:40:25.448731Z
author: Brian McCallister
priority: medium
tags:
- bug
- main.rs
---

Several `.unwrap()` calls in main.rs could panic on edge cases:

1. **Line 116**: `names::Generator::default().next().unwrap()` - session name generator could theoretically fail
2. **Line 120**: `socket_path.to_str().unwrap()` - non-UTF8 paths would panic

**Locations**: main.rs:116, main.rs:120

**Suggested fixes:**
- Line 116: Use `.context()` or provide a fallback session name
- Line 120: Use `.to_str().context("Invalid path encoding")?` for proper error propagation

These are low-probability failures but violate the pattern of using `anyhow::Result` for proper error handling throughout.

---
# Log: 2026-01-28T04:40:25Z Brian McCallister

Created task.
