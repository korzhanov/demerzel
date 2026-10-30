-- Схема состояния Demerzel, версия 1 (#7).

CREATE TABLE projects (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    path        TEXT NOT NULL UNIQUE,
    parent_id   INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE processes (
    id          INTEGER PRIMARY KEY,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    command     TEXT NOT NULL,
    pid         INTEGER,
    status      TEXT NOT NULL,
    exit_code   INTEGER,
    started_at  TEXT,
    stopped_at  TEXT
);

CREATE TABLE port_leases (
    port        INTEGER PRIMARY KEY,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    process_id  INTEGER REFERENCES processes(id) ON DELETE SET NULL,
    leased_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    status      TEXT NOT NULL,
    source      TEXT NOT NULL,
    url         TEXT
);

CREATE TABLE agent_runs (
    id          TEXT PRIMARY KEY,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id     TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    status      TEXT NOT NULL,
    worktree    TEXT,
    started_at  TEXT,
    finished_at TEXT
);

CREATE TABLE alerts (
    id          INTEGER PRIMARY KEY,
    severity    TEXT NOT NULL,
    message     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    acked_at    TEXT
);
