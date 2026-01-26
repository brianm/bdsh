---
yatl_version: 1
title: Execute SSH commands via tmux windows
id: s7bsv7kh
created: 2026-01-18T21:40:56.114966Z
updated: 2026-01-26T05:00:05.457968Z
author: Brian McCallister
priority: high
tags:
- server-mode
- tmux
blocked_by:
- 14ft0q0f
- vfpfzrkt
- g5038fhj
---

Wire up the server mode to actually execute commands:

1. Parse hosts and command from CLI
2. Create output directory structure
3. Start tmux control session with dedicated socket
4. For each host:
   - Create host subdirectory
   - Generate command script
   - Create tmux window: `new-window -d -n $host 'sh $output_dir/$host/command | tee $output_dir/$host/out.log'`
5. Start client/watch mode in the main tmux window (or separate process)
6. Wait for all windows to complete
7. Clean up (unless --keep)

This is the main integration point that ties together CLI, temp dir, command scripts, and tmux control.

---
# Log: 2026-01-18T21:40:56Z Brian McCallister

Created task.

---
# Log: 2026-01-18T21:41:14Z Brian McCallister

Added blocker: mzmjcgt2

---
# Log: 2026-01-18T21:41:14Z Brian McCallister

Added blocker: 14ft0q0f

---
# Log: 2026-01-18T21:41:14Z Brian McCallister

Added blocker: vfpfzrkt

---
# Log: 2026-01-26T05:00:05Z Brian McCallister

Removed blocker: mzmjcgt2

---
# Log: 2026-01-26T05:00:05Z Brian McCallister

Added blocker: g5038fhj
