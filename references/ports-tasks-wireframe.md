# Ports and tasks dashboard

Existing roots, project tree, process controls and logs remain above the new panels.

## Ports

[Refresh] [Polling interval: 5 seconds]
Port | Project/path | Process | PID | Status | Action
5100 | /code/api    | node    | 123 | ok     | Stop…
22   | —            | sshd    | 456 | foreign| Read only

Conflicts have a red row and explicit status text; foreign sockets have an amber row.
Stop opens an inline confirmation naming the PID and project. Confirm and Cancel are
separate buttons. A foreign process never offers a stop action.

## Tasks

[Project: All / project path] [Refresh tasks]
Source failures appear individually with project names. Successfully loaded projects
remain visible. Columns: To do | In progress | Done | Blocked. Each card shows title,
project and source, and an Open action for its source file or issue URL.
