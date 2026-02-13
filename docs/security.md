# OpenJax Security Model

## Overview

OpenJax is an AI agent framework that executes commands on your behalf. This document outlines the security mechanisms and potential risks.

## Sandbox Modes

### WorkspaceWrite (Default)

The default sandbox mode with the following restrictions:

**Allowed Operations:**
- Read files within the workspace directory
- Execute whitelisted read-only commands: `pwd`, `ls`, `cat`, `rg`, `find`, `head`, `tail`, `wc`, `sed`, `awk`, `echo`, `stat`, `uname`, `which`, `env`, `printf`

**Blocked Operations:**
- Network commands: `curl`, `wget`, `ssh`, `scp`, `nc`, `nmap`, `ping`, `sudo`
- Shell operators: `&&`, `||`, `|`, `;`, `>`, `<`, `` ` ``, `$()`
- Parent directory traversal (`../`)
- Absolute paths outside workspace

### DangerFullAccess

Unrestricted mode - use only in isolated environments.

**WARNING**: This mode allows all shell commands. Never use in production or on machines with sensitive data.

## Approval Policy

| Policy | Behavior |
|--------|----------|
| `always_ask` | Confirm every command before execution |
| `on_request` | Only confirm high-risk commands (default) |
| `never` | Execute without confirmation |

## Path Security

All file operations are validated against:
1. No absolute paths (except in `danger_full_access` mode)
2. No parent directory traversal (`../`)
3. No symlink escape attempts

## Risks

1. **Command Execution**: Even with restrictions, dangerous commands can slip through
2. **File Modification**: `apply_patch` can modify or delete files
3. **Data Exfiltration**: In `danger_full_access` mode, any data can be exfiltrated
4. **Model Misbehavior**: The AI model may generate unexpected commands

## Best Practices

1. Use `workspace_write` mode (default)
2. Use `always_ask` approval for sensitive tasks
3. Run in isolated containers or VMs
4. Review generated commands before execution
5. Monitor agent activity logs

## Disclaimer

OpenJax is provided as-is. Users are responsible for securing their environments and monitoring agent activities.
