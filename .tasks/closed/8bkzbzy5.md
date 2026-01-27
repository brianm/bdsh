---
yatl_version: 1
title: Add watch window as window 0 in tmux session
id: 8bkzbzy5
created: 2026-01-27T02:24:14.465294Z
updated: 2026-01-27T02:25:20.557493Z
author: Brian McCallister
priority: high
tags:
- server-mode
---

Window 0 runs `bdsh watch $output_dir` showing consensus view with live updates.
Host command windows become 1-N instead of 0-(N-1).

User can switch between:
- Window 0: unified consensus view with status
- Windows 1-N: individual host output

This keeps the session useful after commands complete - you can see the summary in window 0.

---
# Log: 2026-01-27T02:24:14Z Brian McCallister

Created task.

---
# Log: 2026-01-27T02:24:19Z Brian McCallister

Started working.

---
# Log: 2026-01-27T02:25:20Z Brian McCallister

Closed: Implemented watch window as window 0 in tmux session. Hosts run in windows 1-N. Added --no-watch flag to disable.
