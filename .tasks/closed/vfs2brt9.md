---
yatl_version: 1
title: 'Refactor: duplicate gutter formatting logic'
id: vfs2brt9
created: 2026-01-28T04:40:42.249300Z
updated: 2026-01-28T04:59:09.173862Z
author: Brian McCallister
priority: medium
tags:
- refactor
- duplication
---

The gutter width calculation and formatting logic is duplicated between two locations:

1. **watch/mod.rs:349-396** - `render_text_consensus()` 
2. **watch/consensus.rs:412-439, 493-509** - `build_display_lines()`

Both calculate `max_gutter_width` the same way and format host counts identically:
- Single host: show hostname
- Multiple hosts collapsed: show `[N]`  
- Multiple hosts expanded: show comma-joined list

**Suggestion**: Extract a shared helper function or struct for gutter formatting:
```rust
struct GutterFormatter {
    max_width: usize,
}

impl GutterFormatter {
    fn format(&self, hosts: &[String], expanded: bool) -> String { ... }
}
```

This would reduce ~80 lines of duplicated logic to a single reusable component.

---
# Log: 2026-01-28T04:40:42Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:59:09Z Brian McCallister

Closed: Extracted format_gutter(), gutter_width(), and max_gutter_width() helper functions; removed ~50 lines of duplicate code
