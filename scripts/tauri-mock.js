// Мок Tauri invoke для скриншотов и браузерных проверок (#150).
// Подставляется через page.addInitScript: отвечает фикстурами на вызовы UI, без Rust-бэкенда.
// Неизвестные команды получают null — UI показывает пустое состояние, а не падает.
(() => {
  const PROJECTS = [
    { name: "parser", path: "/Users/demo/code/parser", children: [] },
    { name: "hrh-app", path: "/Users/demo/code/hrh-app", children: [] },
    { name: "savethedate", path: "/Users/demo/code/savethedate", children: [] },
  ];
  const fixtures = {
    get_settings: { scan_roots: ["/Users/demo/code"], alerts: undefined, agents: undefined },
    scan_projects: PROJECTS,
    list_processes: [
      { id: 1, project: "parser", command: "bun run dev", status: "Running", pid: 4312, port: 5173 },
      { id: 2, project: "hrh-app", command: "bun run dev", status: { Exited: 1 }, pid: null, port: null },
    ],
    list_ports: [
      { port: 5173, project: "parser", command: "node", pid: 4312, status: "ok", managed_id: 1 },
      { port: 3000, project: null, command: "rapportd", pid: 611, status: "ok", managed_id: null },
    ],
    project_tasks: [
      { id: "t1", project: "parser", title: "Проверить парсер [ссылка](https://example.com)", status: "todo", source: "github", url: null },
      { id: "t2", project: "hrh-app", title: "Обновить лендинг", status: "in_progress", source: "notion", url: null },
    ],
    list_agent_runs: [],
    list_available_agents: [],
    automation_statuses: [],
    load_automations: { discovered: [], selected: [], restart: { enabled: false, max_retries: 3 } },
    resolve_preferred_agent: null,
    resolve_dev_command: "bun run dev",
    list_dir_children: [],
    process_logs: [],
  };
  const callbacks = new Map();
  let nextId = 1;
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd) => {
      if (cmd in fixtures) return structuredClone(fixtures[cmd]);
      if (cmd.startsWith("plugin:")) return 0;
      return null;
    },
    transformCallback: (cb) => {
      const id = nextId++;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (p) => p,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  };
})();
