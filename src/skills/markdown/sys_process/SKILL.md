---
name: sys-process
description: When the user wants to list running processes or check what's running on the system.
---

# System Process

## When to use
- User wants to see running processes
- User wants to check if a specific process is running
- User asks "!sys_process" or "!sys_process <filter>"

## How to use

### Discord
Run the `sys_process` skill via the Discord command:
`!skill sys_process` — list top 20 processes
`!skill sys_process chrome` — filter by name

### Web API
`POST /api/skills/sys_process` with body `<filter>` (optional)

## Notes
- Lists up to 20 processes by default
- Filter is case-insensitive
- Shows PID, user, CPU%, memory%, VSZ, RSS, command
