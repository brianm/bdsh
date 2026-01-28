---
yatl_version: 1
title: 'Bug: set -e prevents exit code capture in command script'
id: 4fhvsmma
created: 2026-01-28T04:40:22.267222Z
updated: 2026-01-28T04:40:22.267222Z
author: Brian McCallister
priority: high
tags:
- bug
- main.rs
---

In `generate_command_script()` (main.rs:227-259), the script uses `set -e` which causes the script to exit immediately on any non-zero exit code. However, the script then tries to capture the exit code with `EXIT_CODE=$?` after `ssh -t`. If ssh fails, `set -e` causes the script to exit before capturing the exit code.

**Location**: main.rs:227-259

**Current code:**
```bash
set -e
...
ssh -t {host} '{escaped_command}'
EXIT_CODE=$?
```

**Fix options:**
1. Remove `set -e` entirely
2. Use `|| true` after ssh to prevent exit: `ssh -t {host} '{escaped_command}' || true`
3. Use `set +e` before ssh and `set -e` after capturing exit code

The current behavior means that when ssh fails, the status file never gets updated from "running" to "failed".

---
# Log: 2026-01-28T04:40:22Z Brian McCallister

Created task.
