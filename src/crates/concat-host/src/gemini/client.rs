// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Client for Google Gemini Generative Language API.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MODEL: &str = "gemini-3-flash-preview";
pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// A message turn or context entry in Gemini format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            parts: vec![Part {
                text: Some(text.into()),
                ..Default::default()
            }],
        }
    }

    pub fn model(text: impl Into<String>) -> Self {
        Self {
            role: "model".into(),
            parts: vec![Part {
                text: Some(text.into()),
                ..Default::default()
            }],
        }
    }

    pub fn function_response(name: impl Into<String>, response: Value) -> Self {
        let resp_obj = if response.is_object() {
            response
        } else {
            serde_json::json!({ "result": response })
        };
        Self {
            role: "user".into(),
            parts: vec![Part {
                function_response: Some(FunctionResponse {
                    name: name.into(),
                    response: resp_obj,
                    ..Default::default()
                }),
                ..Default::default()
            }],
        }
    }

    pub fn function_responses(responses: Vec<(String, Value)>) -> Self {
        let parts = responses
            .into_iter()
            .map(|(name, response)| {
                let resp_obj = if response.is_object() {
                    response
                } else {
                    serde_json::json!({ "result": response })
                };
                Part {
                    function_response: Some(FunctionResponse {
                        name,
                        response: resp_obj,
                    }),
                    ..Default::default()
                }
            })
            .collect();
        Self {
            role: "user".into(),
            parts,
        }
    }
}

/// A part of a content turn.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "thought_signature")]
    pub thought_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Part {
    pub fn is_valid(&self) -> bool {
        self.text.as_ref().map_or(false, |t| !t.trim().is_empty())
            || self.function_call.is_some()
            || self.function_response.is_some()
            || self.thought_signature.is_some()
            || !self.extra.is_empty()
    }
}

/// A model call to a tool.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    pub name: String,
    pub args: Value,
}

/// An execution result of a tool.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    pub name: String,
    pub response: Value,
}

/// System instruction wrapper.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInstruction {
    pub parts: Vec<SystemPart>,
}

impl SystemInstruction {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            parts: vec![SystemPart { text: text.into() }],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPart {
    pub text: String,
}

/// Tools declaration for Gemini.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Declaration of a single function.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// The payload sent to `generateContent`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<SystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<Value>,
}

/// The response from `generateContent`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    pub candidates: Option<Vec<Candidate>>,
    pub error: Option<ApiErrorDetails>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub content: Option<Content>,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDetails {
    pub code: Option<i64>,
    pub message: Option<String>,
    pub status: Option<String>,
}

/// Low-level HTTP client for Gemini.
pub struct GeminiClient {
    pub token: String,
    /// Cloud project billed/enabled for the Generative Language API. Only sent
    /// with OAuth user tokens (an API key already carries its own project).
    pub quota_project: Option<String>,
}

impl GeminiClient {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            quota_project: None,
        }
    }

    /// Sets the project sent as `x-goog-user-project` for OAuth requests.
    pub fn with_quota_project(mut self, project: Option<String>) -> Self {
        self.quota_project = project;
        self
    }

    /// Sends a `generateContent` request to the specified Gemini model.
    pub fn generate_content(
        &self,
        model: &str,
        request: &GenerateContentRequest,
    ) -> Result<GenerateContentResponse, String> {
        let is_api_key = self.token.starts_with("AIza") || !self.token.contains('.');

        // Filter and clean contents: ensure no empty parts without initialized oneof data
        let clean_contents: Vec<Content> = request
            .contents
            .iter()
            .filter_map(|c| {
                let valid_parts: Vec<Part> = c
                    .parts
                    .iter()
                    .filter(|p| p.is_valid())
                    .cloned()
                    .collect();
                if valid_parts.is_empty() {
                    None
                } else {
                    Some(Content {
                        role: c.role.clone(),
                        parts: valid_parts,
                    })
                }
            })
            .collect();

        let req_to_send = GenerateContentRequest {
            contents: clean_contents,
            system_instruction: request.system_instruction.clone(),
            tools: request.tools.clone(),
            generation_config: request.generation_config.clone(),
        };

        // List of candidate models to try in case of transient 503 high demand, 429 quota, or 404 deprecation
        let mut candidates = vec![model];
        for fallback in &["gemini-3-flash-preview", "gemini-3.1-flash-lite"] {
            if !candidates.contains(fallback) {
                candidates.push(fallback);
            }
        }

        let mut last_error = String::new();

        for candidate in candidates {
            for attempt in 0..2 {
                if attempt > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(800));
                }

                let url = if is_api_key {
                    format!("{GEMINI_API_BASE}/{candidate}:generateContent?key={}", self.token)
                } else {
                    format!("{GEMINI_API_BASE}/{candidate}:generateContent")
                };

                let mut req = ureq::post(&url)
                    .timeout(std::time::Duration::from_secs(60))
                    .set("Content-Type", "application/json");

                if !is_api_key {
                    req = req.set("Authorization", &format!("Bearer {}", self.token));
                    if let Some(project) = &self.quota_project {
                        req = req.set("x-goog-user-project", project);
                    }
                }

                match req.send_json(&req_to_send) {
                    Ok(resp) => {
                        let parsed: GenerateContentResponse = resp
                            .into_json()
                            .map_err(|e| format!("Failed to parse Gemini response: {e}"))?;

                        if let Some(err) = parsed.error {
                            last_error = format!(
                                "Gemini API returned error: {}",
                                err.message.unwrap_or_else(|| "Unknown error".into())
                            );
                            continue;
                        }

                        return Ok(parsed);
                    }
                    Err(ureq::Error::Status(code, resp)) => {
                        let text = resp.into_string().unwrap_or_default();
                        last_error = format!("Gemini API error (HTTP {code}): {text}");
                        if code == 503 {
                            // High demand: retry same candidate
                            continue;
                        } else if code == 429 || code == 404 {
                            // Daily quota exhausted or model deprecated: switch immediately to next candidate
                            break;
                        } else {
                            break;
                        }
                    }
                    Err(ureq::Error::Transport(err)) => {
                        last_error = format!("Network error communicating with Gemini: {err}");
                        continue;
                    }
                }
            }
        }

        Err(last_error)
    }
}
