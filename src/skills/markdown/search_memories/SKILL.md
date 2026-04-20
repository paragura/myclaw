---
name: search-memories
description: When the user wants to search stored memories or find information myclaw has learned.
---

# Search Memories

## When to use
- User asks about things myclaw has learned
- User wants to find specific memories
- User asks "!search <query>" or "!memories"

## How to use

### Discord
Run the `search_memories` skill via the Discord command:
`!skill search_memories <query>`

Or use the built-in search command:
`!search <query>`

### Web API
`POST /api/skills/search_memories` with body `<query>`

## Notes
- Searches across all memory categories
- Returns up to 10 results sorted by importance
- Use `!memories <category>` to list memories by category
