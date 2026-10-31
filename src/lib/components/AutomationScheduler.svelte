<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AutomationStatus, AutomationSchedule, AutomationsView, Project } from "$lib/types";

  let { projects }: { projects: Project[] } = $props();
  const schedulesByPath = new Map<string, AutomationSchedule[]>();
  const lastRunByKey = new Map<string, number>();
  const busy = new Set<string>();
  const lastConfigLoad = new Map<string, number>();

  function key(path: string, source: string, name: string) {
    return `${path}\u0000${source}\u0000${name}`;
  }

  async function loadSchedules(project: Project) {
    const result = await invoke<AutomationsView>("load_automations", { path: project.path });
    const selected = new Set(
      result.selected.map((item) => `${item.source}\u0000${item.name}`),
    );
    const runnable = new Set(
      result.discovered
        .filter((item) => item.command)
        .map((item) => `${item.source}\u0000${item.name}`),
    );
    schedulesByPath.set(
      project.path,
      (result.schedules ?? []).filter((schedule) => {
        const id = `${schedule.source}\u0000${schedule.name}`;
        return schedule.enabled && selected.has(id) && runnable.has(id);
      }),
    );
    lastConfigLoad.set(project.path, Date.now());
  }

  async function poll(projectsSnapshot: Project[]) {
    for (const project of projectsSnapshot) {
      if (Date.now() - (lastConfigLoad.get(project.path) ?? 0) > 30_000) {
        try {
          await loadSchedules(project);
        } catch {
          continue;
        }
      }
      const schedules = schedulesByPath.get(project.path) ?? [];
      const due = schedules.filter((s) => s.enabled && s.interval_minutes > 0);
      if (due.length === 0) continue;

      let statuses: AutomationStatus[];
      try {
        statuses = await invoke<AutomationStatus[]>("automation_statuses", { path: project.path });
      } catch {
        continue;
      }

      for (const schedule of due) {
        const runKey = key(project.path, schedule.source, schedule.name);
        if (busy.has(runKey)) continue;
        const previous = lastRunByKey.get(runKey);
        if (previous === undefined) {
          // Start the first interval from app launch, instead of running immediately.
          lastRunByKey.set(runKey, Date.now());
          continue;
        }
        if (Date.now() - previous < schedule.interval_minutes * 60_000) continue;
        const status = statuses.find((s) => s.source === schedule.source && s.name === schedule.name);
        if (status?.status === "Running" || status?.status === "Starting") continue;

        busy.add(runKey);
        lastRunByKey.set(runKey, Date.now());
        try {
          await invoke("start_automation", {
            project: project.name,
            path: project.path,
            source: schedule.source,
            name: schedule.name,
          });
        } catch (error) {
          console.error(`[automation] scheduled run failed for ${project.name}/${schedule.name}:`, error);
        } finally {
          busy.delete(runKey);
        }
      }
    }
  }

  $effect(() => {
    const snapshot = [...projects];
    const signature = snapshot.map((p) => `${p.name}\u0000${p.path}`).join("\u0001");
    void signature;
    for (const project of snapshot) void loadSchedules(project).catch(() => {});
    const onScheduleUpdated = (event: Event) => {
      const path = (event as CustomEvent<{ path: string }>).detail?.path;
      const project = snapshot.find((item) => item.path === path);
      if (project) void loadSchedules(project).catch(() => {});
    };
    window.addEventListener("demerzel:automation-schedule-updated", onScheduleUpdated);
    const timer = setInterval(() => void poll(snapshot), 10_000);
    return () => {
      clearInterval(timer);
      window.removeEventListener("demerzel:automation-schedule-updated", onScheduleUpdated);
    };
  });
</script>

<span class="hidden" aria-hidden="true"></span>
