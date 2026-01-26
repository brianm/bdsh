---
yatl_version: 1
title: Auto-detect content format (JSON/YAML/tabular)
id: fg7vd2gz
created: 2026-01-26T04:57:57.301111Z
updated: 2026-01-26T04:58:09.762533Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- 7a0a2txt
---

Detect format based on first non-whitespace character:
- `{` or `[` → JSON
- `---` → YAML  
- Otherwise → line-oriented tabular

```rust
enum Format { Json, Yaml, Tabular }

fn detect_format(content: &str) -> Format {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        Format::Json
    } else if trimmed.starts_with("---") {
        Format::Yaml
    } else {
        Format::Tabular
    }
}
```

Route to appropriate parser based on detected format.

---
# Log: 2026-01-26T04:57:57Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:58:09Z Brian McCallister

Added blocker: 7a0a2txt
