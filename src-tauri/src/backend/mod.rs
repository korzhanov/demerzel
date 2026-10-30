//! Бэкенд Demerzel. Модули не зависят от Tauri: логика тестируется без GUI.
pub mod agents;
pub mod config;
pub mod logs;
pub mod alerts;
pub mod notify;
pub mod speak;
pub mod ports;
pub mod process;
pub mod registry;
pub mod scanner;
pub mod settings;
pub mod storage;
pub mod summary;
pub mod pulse;
pub mod icloud_sync;
pub mod widgetkit_fallback;
pub mod siri_config;
pub mod tasks;
pub mod widget;
pub mod widget_refresh;

pub mod github_tasks;
pub mod notion_tasks;
pub mod ecosystem;
pub mod docker;
pub mod cli;
