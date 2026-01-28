---
yatl_version: 1
title: 'Refactor: Extract shell script template from generate_command_script'
id: 11v4wzpj
created: 2026-01-28T04:41:22.585051Z
updated: 2026-01-28T04:41:22.585051Z
author: Brian McCallister
priority: low
tags:
- refactor
- main.rs
---

`generate_command_script()` in main.rs:218-269 embeds a complex shell script as a format string. This makes it hard to:
- Test the shell script logic independently
- See the actual script without Rust string escaping
- Maintain the script

**Current approach:**
```rust
let script = format!(
    r#"#!/bin/sh
# bdsh command wrapper for {host}
set -e
...
"#,
    host = host,
    ...
);
```

**Options:**
1. **Include file**: Store template in `src/command_template.sh`, use `include_str!`
2. **Separate function**: Keep in Rust but extract template generation
3. **Document inline**: At minimum, add comments explaining the script's purpose

The shell script handles:
- Status file lifecycle (running -> success/failed)
- Meta.json generation with timing
- Exit code capture

This logic is important but buried in a format string.

**Location**: main.rs:218-269

---
# Log: 2026-01-28T04:41:22Z Brian McCallister

Created task.
