# Rust, Tauri, and Svelte

Status: Accepted

## Context
Casklet needs a safe shared core, a native Windows shell, a CLI, and an accessible UI.

## Decision
Use stable Rust 1.98.1 edition 2024, Tauri 2.11, and Svelte 5 with strict TypeScript and Vite.

## Consequences
The WebView receives narrow typed IPC capabilities; no generic shell, filesystem, SQL, or HTTP access is exposed.

## Alternatives considered
Electron, a native-only UI, and a browser-served frontend were rejected for v0.1.
