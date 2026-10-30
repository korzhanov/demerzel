//! Бэкенд Demerzel. Модули не зависят от Tauri: логика тестируется без GUI.
pub mod agents;
pub mod config;
pub mod logs;
pub mod alerts;
pub mod notify;
pub mod ports;
pub mod process;
pub mod registry;
pub mod scanner;
pub mod settings;
pub mod storage;
pub mod summary;
pub mod tasks;
pub mod widget;
pub mod widget_refresh;

pub mod github_tasks;
pub mod ecosystem;
pub mod docker;
pub mod cli;
