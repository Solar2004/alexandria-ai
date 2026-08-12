//! alx-mcp — Protocolo MCP (JSON-RPC 2.0 sobre stdio) + catálogo de tools.
//!
//! Fase 1: subset funcional del protocolo sin servidores externos reales.
//! - [`catalog`]: catálogo central de tools que expone el motor.
//! - [`server`]: server JSON-RPC 2.0 sobre stdio (`initialize`, `tools/list`, `tools/call`).
//! - [`client`]: registro de clientes MCP externos (stub de discovery).

pub mod catalog;
pub mod client;
pub mod server;
