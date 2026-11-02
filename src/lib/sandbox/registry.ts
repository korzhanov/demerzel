// Реестр историй песочницы /test (#sandbox), по уровням atomic design.
// Чтобы добавить компонент: запись в `stories` с `level`, `component` и `controls`.
import type { Component } from "svelte";
import ButtonDemo from "./ButtonDemo.svelte";
import BadgeDemo from "./BadgeDemo.svelte";
import CheckboxRow from "./CheckboxRow.svelte";
import FieldRow from "./FieldRow.svelte";
import PortsPanel from "$lib/components/PortsPanel.svelte";

export type Level = "atom" | "molecule" | "organism";

export type Control =
  | { key: string; type: "text"; label: string }
  | { key: string; type: "number"; label: string; min?: number; max?: number }
  | { key: string; type: "boolean"; label: string }
  | { key: string; type: "select"; label: string; options: string[] };

export interface Story {
  id: string;
  level: Level;
  title: string;
  component: Component<any>;
  props: Record<string, unknown>;
  controls: Control[];
}

export const stories: Story[] = [
  {
    id: "atom-button",
    level: "atom",
    title: "Button",
    component: ButtonDemo,
    props: { label: "Запустить", variant: "default", size: "default", disabled: false },
    controls: [
      { key: "label", type: "text", label: "Текст" },
      { key: "variant", type: "select", label: "Вид", options: ["default", "outline", "secondary", "ghost", "destructive", "link"] },
      { key: "size", type: "select", label: "Размер", options: ["sm", "default", "lg"] },
      { key: "disabled", type: "boolean", label: "Недоступна" },
    ],
  },
  {
    id: "atom-badge",
    level: "atom",
    title: "Badge",
    component: BadgeDemo,
    props: { label: "упал", variant: "destructive" },
    controls: [
      { key: "label", type: "text", label: "Текст" },
      { key: "variant", type: "select", label: "Вид", options: ["default", "secondary", "destructive", "outline"] },
    ],
  },
  {
    id: "molecule-checkbox-row",
    level: "molecule",
    title: "CheckboxRow",
    component: CheckboxRow,
    props: { label: "Сканировать папку", checked: true, disabled: false },
    controls: [
      { key: "label", type: "text", label: "Подпись" },
      { key: "checked", type: "boolean", label: "Отмечено" },
      { key: "disabled", type: "boolean", label: "Недоступно" },
    ],
  },
  {
    id: "molecule-field-row",
    level: "molecule",
    title: "FieldRow",
    component: FieldRow,
    props: { label: "Интервал, сек", value: "5", disabled: false },
    controls: [
      { key: "label", type: "text", label: "Подпись" },
      { key: "value", type: "text", label: "Значение" },
      { key: "disabled", type: "boolean", label: "Недоступно" },
    ],
  },
  {
    id: "organism-ports-panel",
    level: "organism",
    title: "PortsPanel",
    component: PortsPanel,
    props: {
      ports: [
        { port: 5173, project: "parser", command: "node", pid: 4312, status: "ok", managed_id: 1 },
        { port: 3000, project: null, command: "rapportd", pid: 611, status: "ok", managed_id: null },
      ],
      portsError: "",
      portsBusy: false,
      portInterval: 5,
      onRefresh: () => {},
      onStopManaged: async () => {},
    },
    controls: [
      { key: "portsBusy", type: "boolean", label: "Загрузка" },
      { key: "portsError", type: "text", label: "Ошибка" },
      { key: "portInterval", type: "number", label: "Интервал опроса", min: 1, max: 60 },
    ],
  },
];

export const LEVELS: Level[] = ["atom", "molecule", "organism"];
