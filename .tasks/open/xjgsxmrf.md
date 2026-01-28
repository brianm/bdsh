---
yatl_version: 1
title: 'Refactor: run_command() function is too long'
id: xjgsxmrf
created: 2026-01-28T04:41:22.648879Z
updated: 2026-01-28T04:41:22.648879Z
author: Brian McCallister
priority: low
tags:
- refactor
- main.rs
---

The `run_command()` function in main.rs:84-202 is ~120 lines and handles multiple concerns:

1. **Host resolution** (line 93)
2. **Output directory creation** (lines 95-105)
3. **Host directory creation** (lines 107-110)
4. **Session naming** (lines 113-116)
5. **Socket path setup** (lines 118-120)
6. **Window offset calculation** (lines 126-128)
7. **Session creation** (lines 130-146)
8. **Host window creation** (lines 148-173)
9. **Window selection** (lines 175-180)
10. **Tmux attach** (lines 182-189)
11. **Cleanup logic** (lines 191-199)

**Suggestion**: Extract into smaller functions:
- `setup_output_dir()` - creates output directory structure
- `create_tmux_session()` - creates session and windows
- `attach_and_cleanup()` - handles attach and cleanup

This would make each function ~30-40 lines and easier to test.

**Location**: main.rs:84-202

---
# Log: 2026-01-28T04:41:22Z Brian McCallister

Created task.
