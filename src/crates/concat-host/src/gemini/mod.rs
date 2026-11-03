// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Gemini AI assistant integration for Concat.
//!
//! Provides Google OAuth 2.0 PKCE authentication, Gemini Generative Language API
//! client with Function Calling, project tool execution, and an intelligent editing agent.

#![allow(missing_docs)]

pub mod agent;
pub mod client;
pub mod oauth;
pub mod tools;

pub use agent::{ActionStatus, ChatMessage, ExecutionMode, GeminiAgent};
pub use client::GeminiClient;
pub use oauth::{GeminiAuth, OAuthFlow};
pub use tools::{describe_action, execute_tool, get_tools, ProposedAction};
