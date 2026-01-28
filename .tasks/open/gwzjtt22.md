---
yatl_version: 1
title: 'Improve: Error message should mention --watch mode'
id: gwzjtt22
created: 2026-01-28T04:41:38.919835Z
updated: 2026-01-28T04:41:38.919835Z
author: Brian McCallister
priority: low
tags:
- ux
- main.rs
---

When no command is provided, the error message shows:
```
Command required: bdsh [source] [filter] -- command
```

This doesn't mention `--watch` mode as an alternative.

**Location**: main.rs:56-58

**Suggestion**: Update to:
```rust
anyhow::bail!("Command required: bdsh [source] [filter] -- command\nOr use: bdsh --watch <output-dir> to watch existing output");
```

This would help users who might be trying to watch output from a previous run.

---
# Log: 2026-01-28T04:41:38Z Brian McCallister

Created task.
