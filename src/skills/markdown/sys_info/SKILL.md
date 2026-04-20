---
name: sys-info
description: When the user wants to get system information such as OS, disk, memory details.
---

# System Info

## When to use
- User asks about system information
- User wants to check disk usage, memory, or OS details
- User asks "!sys_info" or "!sys_info disk/memory/os"

## How to use

### Discord
Run the `sys_info` skill via the Discord command:
`!skill sys_info` — get all info
`!skill sys_info disk` — disk info only
`!skill sys_info memory` — memory info only
`!skill sys_info os` — OS info only

### Web API
`POST /api/skills/sys_info` with body `<info_type>` (optional: "disk", "memory", "os")

## Notes
- macOS-specific commands are used (df, vm_stat)
- On non-macOS systems, limited information is available
