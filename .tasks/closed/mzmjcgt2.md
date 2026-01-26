---
yatl_version: 1
title: Add CLI argument parsing with clap
id: mzmjcgt2
created: 2026-01-18T21:39:54.094358Z
updated: 2026-01-26T04:59:34.786916Z
author: Brian McCallister
priority: high
tags:
- foundation
- cli
---

Parse CLI arguments: hosts (positional or from file), command to run, output directory (optional), flags for keeping output, verbose mode.

Arguments needed:
- hosts: comma-separated or @file for host list
- command: the command to execute on all hosts
- --output-dir / -o: custom output directory (default: temp)
- --keep / -k: preserve output directory on exit
- --socket / -S: custom tmux socket path
- --verbose / -v: verbose output

Should integrate with existing Job struct.

---
# Log: 2026-01-18T21:39:54Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:59:34Z Brian McCallister

Closed: Broken down into subtasks: 3132nkry, 31n7nrrq, 45dn6pzp, 7a0a2txt, fg7vd2gz, d8kq9asx, 3d2ymbad, hqxt0dwn, g5038fhj
