// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The Gemini AI Assistant pane controller: handles chat turns, Google OAuth login,
//! and function calling tools that automate video editing.

use slint::{ComponentHandle, ModelRc, VecModel};

use concat_host::gemini::agent::{ActionStatus, ExecutionMode, GeminiAgent, ChatMessage};
use concat_host::gemini::client::{Content, FunctionCall, GeminiClient, GenerateContentResponse};
use concat_host::gemini::oauth::{GeminiAuth, OAuthExchangeResult, OAuthFlow};
use concat_host::gemini::tools::{
    apply_generated_image, describe_action, execute_tool, fetch_ai_image, ProposedAction,
};

use crate::host::spawn;
use crate::panes::Msg;
use crate::studio::Studio;
use crate::ui::{AssistantMessageData, Editor};

use serde::{Deserialize, Serialize};

/// Persistent assistant chat and conversation history tied to a specific project.
#[derive(Serialize, Deserialize, Default)]
pub struct ProjectAssistantData {
    pub messages: Vec<ChatMessage>,
    pub history: Vec<Content>,
}

/// Events that can be posted to the Assistant pane.
pub enum AssistantMsg {
    ProjectOpened,
    Login,
    LoginComplete(Result<OAuthExchangeResult, String>),
    Logout,
    ModelChanged(String),
    ModeChanged(bool),
    SendMessage(String),
    ModelTurnDone(Result<GenerateContentResponse, String>),
    ImageDownloaded {
        tool_name: String,
        call_args: serde_json::Value,
        prompt: String,
        start: f64,
        duration: f64,
        track_id: Option<String>,
        result: Result<Vec<u8>, String>,
    },
    ApproveAction(String),
    RejectAction(String),
    Clear,
}

impl std::fmt::Debug for AssistantMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectOpened => write!(f, "ProjectOpened"),
            Self::Login => write!(f, "Login"),
            Self::LoginComplete(res) => write!(f, "LoginComplete({:?})", res.is_ok()),
            Self::Logout => write!(f, "Logout"),
            Self::ModelChanged(m) => write!(f, "ModelChanged({m})"),
            Self::ModeChanged(auto) => write!(f, "ModeChanged(autonomous={auto})"),
            Self::SendMessage(txt) => write!(f, "SendMessage({txt})"),
            Self::ModelTurnDone(res) => write!(f, "ModelTurnDone({:?})", res.is_ok()),
            Self::ImageDownloaded { prompt, result, .. } => {
                write!(f, "ImageDownloaded(\"{prompt}\", ok={})", result.is_ok())
            }
            Self::ApproveAction(id) => write!(f, "ApproveAction({id})"),
            Self::RejectAction(id) => write!(f, "RejectAction({id})"),
            Self::Clear => write!(f, "Clear"),
        }
    }
}

pub struct AssistantController {
    pub auth: GeminiAuth,
    pub agent: GeminiAgent,
    pub busy: bool,
    pub status: String,
    pub pending_action: Option<ProposedAction>,
    pub current_project_path: Option<String>,
}

impl AssistantController {
    pub fn new(dirs: &concat_host::AppDirs) -> Self {
        let auth = GeminiAuth::load(dirs);
        Self {
            auth,
            agent: GeminiAgent::new(),
            busy: false,
            status: String::new(),
            pending_action: None,
            current_project_path: None,
        }
    }

    pub fn save_project_history(&self, project_path: &str) {
        let history_file = std::path::Path::new(project_path).join("assistant_history.json");
        let data = ProjectAssistantData {
            messages: self.agent.messages.clone(),
            history: self.agent.history.clone(),
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&history_file, json_str);
        }
    }

    pub fn load_project_history(project_path: &str) -> Option<(Vec<ChatMessage>, Vec<Content>)> {
        let history_file = std::path::Path::new(project_path).join("assistant_history.json");
        if history_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&history_file) {
                if let Ok(data) = serde_json::from_str::<ProjectAssistantData>(&content) {
                    return Some((data.messages, data.history));
                }
            }
        }
        None
    }

    pub fn auto_save_current_history(&self) {
        if let Some(path) = &self.current_project_path {
            self.save_project_history(path);
        }
    }

    pub fn switch_project(&mut self, new_path: Option<&str>) {
        if self.current_project_path.as_deref() == new_path {
            return;
        }

        // Save current history for previous project if any
        if let Some(old_path) = &self.current_project_path {
            self.save_project_history(old_path);
        }

        self.current_project_path = new_path.map(|s| s.to_string());

        // Load history for the newly active project
        if let Some(path) = &self.current_project_path {
            if let Some((msgs, hist)) = Self::load_project_history(path) {
                self.agent.messages = msgs;
                self.agent.history = hist;
            } else {
                self.agent.clear();
            }
        } else {
            self.agent.clear();
        }

        self.pending_action = None;
        self.busy = false;
        self.status.clear();
    }

    /// Handles a message directed to the assistant.
    pub fn update(&mut self, msg: AssistantMsg, studio: &mut Studio) {
        let active_path = studio.session.as_ref().map(|s| s.path().to_string());
        self.switch_project(active_path.as_deref());

        match msg {
            AssistantMsg::ProjectOpened => {
                // Project switch already handled above!
            }
            AssistantMsg::Login => {
                let client_id = self.auth.client_id().to_string();
                let flow = match OAuthFlow::start(&client_id) {
                    Ok(f) => f,
                    Err(err) => {
                        studio.notify(&format!("Error iniciando OAuth: {err}"), true);
                        return;
                    }
                };

                // Open browser
                if let Err(err) = crate::platform::open_browser(&flow.auth_url) {
                    studio.notify(&format!("No se pudo abrir el navegador: {err}"), true);
                    return;
                }

                self.busy = true;
                self.status = "Esperando autorización en el navegador...".into();

                // Spawn worker to wait for callback
                spawn(
                    move || flow.wait_and_exchange(&client_id),
                    |studio, _app, _models, result| {
                        studio.handle(Msg::Assistant(AssistantMsg::LoginComplete(result)));
                    },
                );
            }

            AssistantMsg::LoginComplete(result) => {
                self.busy = false;
                self.status.clear();

                match result {
                    Ok(exchanged) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);

                        self.auth.access_token = Some(exchanged.access_token);
                        self.auth.refresh_token = exchanged.refresh_token;
                        self.auth.expires_at = Some(now + exchanged.expires_in);
                        self.auth.user_email = exchanged.email;
                        self.auth.user_name = exchanged.name;

                        let _ = self.auth.save(&studio.host.dirs);
                        studio.notify("Sesión de Google iniciada correctamente", false);
                    }
                    Err(err) => {
                        studio.notify(&format!("Error en autenticación: {err}"), true);
                    }
                }
            }

            AssistantMsg::Logout => {
                let _ = self.auth.logout(&studio.host.dirs);
                studio.notify("Sesión cerrada", false);
            }

            AssistantMsg::ModelChanged(model) => {
                self.agent.model = model;
            }

            AssistantMsg::ModeChanged(autonomous) => {
                self.agent.mode = if autonomous {
                    ExecutionMode::Autonomous
                } else {
                    ExecutionMode::Copilot
                };
            }

            AssistantMsg::SendMessage(text) => {
                let trimmed = text.trim();
                let maybe_key = if trimmed.starts_with("/key ") {
                    Some(trimmed.strip_prefix("/key ").unwrap().trim())
                } else if trimmed.starts_with("/apikey ") {
                    Some(trimmed.strip_prefix("/apikey ").unwrap().trim())
                } else if trimmed.starts_with("AIza") && trimmed.len() >= 30 && !trimmed.contains(' ') {
                    Some(trimmed)
                } else {
                    None
                };

                if let Some(key) = maybe_key {
                    if !key.is_empty() {
                        self.auth.api_key = Some(key.to_string());
                        let _ = self.auth.save(&studio.host.dirs);
                        studio.notify("✓ Clave de Gemini API guardada y activada con éxito.", false);
                        return;
                    }
                }

                let token = match self.auth.get_valid_token(&studio.host.dirs) {
                    Ok(t) => t,
                    Err(err) => {
                        studio.notify(&err, true);
                        return;
                    }
                };

                if studio.session.is_none() {
                    studio.notify("No hay ningún proyecto abierto actualmente.", true);
                    return;
                }

                self.busy = true;
                self.status = "Consultando a Gemini...".into();

                self.agent.record_user_message(&text);
                self.auto_save_current_history();

                let client = GeminiClient::new(token).with_quota_project(self.auth.quota_project());
                let model = self.agent.model.clone();
                let req = self.agent.build_request();

                spawn(
                    move || client.generate_content(&model, &req),
                    |studio, _app, _models, result| {
                        studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                    },
                );
            }

            AssistantMsg::ModelTurnDone(result) => {
                let token = match self.auth.get_valid_token(&studio.host.dirs) {
                    Ok(t) => t,
                    Err(err) => {
                        self.busy = false;
                        self.status.clear();
                        studio.notify(&err, true);
                        return;
                    }
                };

                let client = GeminiClient::new(token).with_quota_project(self.auth.quota_project());

                let response = match result {
                    Ok(r) => r,
                    Err(err) => {
                        self.busy = false;
                        self.status.clear();
                        if err.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT") || err.contains("insufficient authentication scopes") {
                            let _ = self.auth.logout(&studio.host.dirs);
                            studio.notify("Token con permisos insuficientes. Se cerró la sesión para renovarla: haz clic en 'Iniciar sesión' o escribe '/key TU_API_KEY'.", true);
                        } else {
                            studio.notify(&format!("Error de Gemini: {err}"), true);
                        }
                        return;
                    }
                };

                let candidate = match response.candidates.and_then(|c| c.into_iter().next()) {
                    Some(c) => c,
                    None => {
                        self.busy = false;
                        self.status.clear();
                        studio.notify("Gemini no devolvió ninguna respuesta.", true);
                        return;
                    }
                };

                let content = match candidate.content {
                    Some(c) => c,
                    None => {
                        self.busy = false;
                        self.status.clear();
                        return;
                    }
                };

                if content.parts.iter().any(|p| p.is_valid()) {
                    self.agent.history.push(content.clone());
                }

                // Check for function calls
                let calls: Vec<&FunctionCall> = content
                    .parts
                    .iter()
                    .filter_map(|p| p.function_call.as_ref())
                    .collect();

                if !calls.is_empty() {
                    // Case 1: Read-only project summary inspection
                    if calls.len() == 1 && calls[0].name.split(':').last().unwrap_or(&calls[0].name) == "get_project_summary" {
                        let call = calls[0];
                        let tool_res = {
                            let session = match studio.session.as_mut() {
                                Some(s) => s,
                                None => {
                                    self.busy = false;
                                    self.status.clear();
                                    return;
                                }
                            };
                            match execute_tool(session, &call.name, &call.args) {
                                Ok(r) => r,
                                Err(err) => {
                                    self.busy = false;
                                    self.status.clear();
                                    studio.notify(&format!("Error leyendo proyecto: {err}"), true);
                                    return;
                                }
                            }
                        };

                        self.agent.history.push(Content::function_response(&call.name, tool_res));
                        self.status = "Analizando proyecto con Gemini...".into();

                        let model = self.agent.model.clone();
                        let req = self.agent.build_request();

                        spawn(
                            move || client.generate_content(&model, &req),
                            |studio, _app, _models, result| {
                                studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                            },
                        );
                        return;
                    }

                    // Case 2: AI image generation (asynchronous network call)
                    if calls.len() == 1 && calls[0].name.split(':').last().unwrap_or(&calls[0].name) == "generate_image" {
                        let call = calls[0];
                        let (description, proposed) = {
                            let session = match studio.session.as_mut() {
                                Some(s) => s,
                                None => {
                                    self.busy = false;
                                    self.status.clear();
                                    return;
                                }
                            };
                            let description = describe_action(session, &call.name, &call.args);
                            let proposed = ProposedAction {
                                tool_name: call.name.clone(),
                                args: call.args.clone(),
                                human_description: description.clone(),
                            };
                            (description, proposed)
                        };

                        if self.agent.mode == ExecutionMode::Copilot {
                            self.busy = false;
                            self.status.clear();

                            let msg_id = format!("prop_{}", self.agent.messages.len() + 1);
                            self.agent.messages.push(ChatMessage {
                                id: msg_id,
                                sender: "gemini".into(),
                                text: format!("He preparado la siguiente acción: {description}"),
                                proposed_action: Some(proposed.clone()),
                                action_status: Some(ActionStatus::Pending),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0),
                            });
                            self.pending_action = Some(proposed);
                            return;
                        }

                        let prompt = call.args.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let start = call.args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let duration = call.args.get("duration_seconds").and_then(|v| v.as_f64()).unwrap_or(5.0);
                        let aspect_ratio = call.args.get("aspect_ratio").and_then(|v| v.as_str()).unwrap_or("16:9");
                        let target_track = call.args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string());

                        let (width, height) = match aspect_ratio {
                            "9:16" => (720, 1280),
                            "1:1" => (1024, 1024),
                            "4:3" => (1024, 768),
                            _ => (1280, 720),
                        };

                        self.status = "Generando imagen con IA (FLUX)...".into();
                        let tool_name = call.name.clone();
                        let call_args = call.args.clone();
                        let prompt_fetch = prompt.clone();

                        spawn(
                            move || fetch_ai_image(&prompt_fetch, width, height),
                            move |studio, _app, _models, res| {
                                studio.handle(Msg::Assistant(AssistantMsg::ImageDownloaded {
                                    tool_name,
                                    call_args,
                                    prompt,
                                    start,
                                    duration,
                                    track_id: target_track,
                                    result: res,
                                }));
                            },
                        );
                        return;
                    }

                    // Case 3: In Copilot mode, propose action card (single action or composite batch)
                    if self.agent.mode == ExecutionMode::Copilot {
                        self.busy = false;
                        self.status.clear();

                        let (proposed, text) = if calls.len() == 1 {
                            let call = calls[0];
                            let description = {
                                let session = match studio.session.as_mut() {
                                    Some(s) => s,
                                    None => return,
                                };
                                describe_action(session, &call.name, &call.args)
                            };
                            let proposed = ProposedAction {
                                tool_name: call.name.clone(),
                                args: call.args.clone(),
                                human_description: description.clone(),
                            };
                            (proposed, format!("He preparado la siguiente acción: {description}"))
                        } else {
                            let session = match studio.session.as_mut() {
                                Some(s) => s,
                                None => return,
                            };
                            let mut descriptions = Vec::new();
                            let mut actions_json = Vec::new();
                            for (idx, call) in calls.iter().enumerate() {
                                let desc = describe_action(session, &call.name, &call.args);
                                descriptions.push(format!("{}. {}", idx + 1, desc));
                                actions_json.push(serde_json::json!({
                                    "tool_name": call.name,
                                    "args": call.args
                                }));
                            }
                            let batch_desc = format!("Lote de {} acciones:\n{}", calls.len(), descriptions.join("\n"));
                            let proposed = ProposedAction {
                                tool_name: "batch".into(),
                                args: serde_json::json!({ "multi_calls": actions_json }),
                                human_description: batch_desc,
                            };
                            (proposed, format!("He preparado el siguiente lote de {} acciones:\n{}", calls.len(), descriptions.join("\n")))
                        };

                        let msg_id = format!("prop_{}", self.agent.messages.len() + 1);
                        self.agent.messages.push(ChatMessage {
                            id: msg_id,
                            sender: "gemini".into(),
                            text,
                            proposed_action: Some(proposed.clone()),
                            action_status: Some(ActionStatus::Pending),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        });
                        self.pending_action = Some(proposed);
                        return;
                    }

                    // Case 4: Autonomous mode - execute all function calls in sequence!
                    let mut responses: Vec<(String, serde_json::Value)> = Vec::new();
                    let mut descriptions: Vec<String> = Vec::new();

                    for call in &calls {
                        let session = match studio.session.as_mut() {
                            Some(s) => s,
                            None => {
                                self.busy = false;
                                self.status.clear();
                                return;
                            }
                        };
                        let desc = describe_action(session, &call.name, &call.args);
                        descriptions.push(desc);

                        let tool_res = match execute_tool(session, &call.name, &call.args) {
                            Ok(r) => r,
                            Err(err) => serde_json::json!({
                                "status": "error",
                                "message": err.to_string(),
                            }),
                        };
                        responses.push((call.name.clone(), tool_res));
                    }

                    studio.after_change();

                    let combined_description = if descriptions.len() == 1 {
                        descriptions[0].clone()
                    } else {
                        format!("Lote ejecutado ({} acciones): {}", descriptions.len(), descriptions.join("; "))
                    };

                    self.agent.record_action_applied(ProposedAction {
                        tool_name: if calls.len() == 1 { calls[0].name.clone() } else { "batch".into() },
                        args: serde_json::json!({ "count": calls.len() }),
                        human_description: combined_description,
                    });

                    self.agent.history.push(Content::function_responses(responses));
                    self.auto_save_current_history();
                    self.status = "Finalizando cambios con Gemini...".into();

                    let model = self.agent.model.clone();
                    let req = self.agent.build_request();

                    spawn(
                        move || client.generate_content(&model, &req),
                        |studio, _app, _models, result| {
                            studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                        },
                    );
                    return;
                }

                // Check for normal text response
                let mut full_text = String::new();
                for part in &content.parts {
                    if let Some(txt) = &part.text {
                        full_text.push_str(txt);
                    }
                }

                if !full_text.is_empty() {
                    self.agent.record_model_text(&full_text);
                    self.auto_save_current_history();
                }

                self.busy = false;
                self.status.clear();
            }

            AssistantMsg::ImageDownloaded {
                tool_name,
                call_args,
                prompt,
                start,
                duration,
                track_id,
                result,
            } => {
                let bytes = match result {
                    Ok(b) => b,
                    Err(err) => {
                        self.busy = false;
                        self.status.clear();
                        studio.notify(&format!("Error descargando imagen de IA: {err}"), true);
                        return;
                    }
                };

                let tool_res = {
                    let session = match studio.session.as_mut() {
                        Some(s) => s,
                        None => {
                            self.busy = false;
                            self.status.clear();
                            return;
                        }
                    };

                    match apply_generated_image(session, &bytes, &prompt, start, duration, track_id) {
                        Ok(r) => r,
                        Err(err) => {
                            self.busy = false;
                            self.status.clear();
                            studio.notify(&format!("Error incrustando imagen en la línea de tiempo: {err}"), true);
                            return;
                        }
                    }
                };

                studio.after_change();
                studio.notify("Imagen generada con IA incrustada en la línea de tiempo", false);

                let description = format!("Generar imagen con IA \"{prompt}\" e incrustarla en el segundo {:.2}s", start);
                let proposed = ProposedAction {
                    tool_name: tool_name.clone(),
                    args: call_args,
                    human_description: description,
                };
                self.agent.record_action_applied(proposed);
                self.agent.history.push(Content::function_response(&tool_name, tool_res));
                self.auto_save_current_history();

                let token = match self.auth.get_valid_token(&studio.host.dirs) {
                    Ok(t) => t,
                    Err(err) => {
                        self.busy = false;
                        self.status.clear();
                        studio.notify(&err, true);
                        return;
                    }
                };

                let client = GeminiClient::new(token).with_quota_project(self.auth.quota_project());
                self.status = "Concluyendo respuesta...".into();

                let model = self.agent.model.clone();
                let req = self.agent.build_request();

                spawn(
                    move || client.generate_content(&model, &req),
                    |studio, _app, _models, result| {
                        studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                    },
                );
            }

            AssistantMsg::ApproveAction(_id) => {
                if let Some(action) = self.pending_action.take() {
                    let clean_name = action.tool_name.split(':').last().unwrap_or(&action.tool_name);
                    if clean_name == "generate_image" {
                        let prompt = action.args.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let start = action.args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let duration = action.args.get("duration_seconds").and_then(|v| v.as_f64()).unwrap_or(5.0);
                        let aspect_ratio = action.args.get("aspect_ratio").and_then(|v| v.as_str()).unwrap_or("16:9");
                        let target_track = action.args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string());

                        let (width, height) = match aspect_ratio {
                            "9:16" => (720, 1280),
                            "1:1" => (1024, 1024),
                            "4:3" => (1024, 768),
                            _ => (1280, 720),
                        };

                        self.busy = true;
                        self.status = "Generando imagen con IA (FLUX)...".into();
                        let tool_name = action.tool_name.clone();
                        let call_args = action.args.clone();
                        let prompt_fetch = prompt.clone();

                        spawn(
                            move || fetch_ai_image(&prompt_fetch, width, height),
                            move |studio, _app, _models, res| {
                                studio.handle(Msg::Assistant(AssistantMsg::ImageDownloaded {
                                    tool_name,
                                    call_args,
                                    prompt,
                                    start,
                                    duration,
                                    track_id: target_track,
                                    result: res,
                                }));
                            },
                        );
                    } else if clean_name == "batch" && action.args.get("multi_calls").is_some() {
                        let multi_calls = action.args.get("multi_calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        let mut responses = Vec::new();
                        {
                            let session = match studio.session.as_mut() {
                                Some(s) => s,
                                None => return,
                            };
                            for item in &multi_calls {
                                let name = item.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                                let a_args = item.get("args").unwrap_or(&serde_json::Value::Null);
                                let res = match execute_tool(session, name, a_args) {
                                    Ok(r) => r,
                                    Err(err) => serde_json::json!({ "status": "error", "message": err.to_string() }),
                                };
                                responses.push((name.to_string(), res));
                            }
                        }
                        studio.after_change();

                        // Mark last message as applied
                        if let Some(last) = self.agent.messages.iter_mut().rev().find(|m| m.proposed_action.is_some()) {
                            last.action_status = Some(ActionStatus::Applied);
                        }

                        self.agent.history.push(Content::function_responses(responses));
                        self.auto_save_current_history();

                        let token = match self.auth.get_valid_token(&studio.host.dirs) {
                            Ok(t) => t,
                            Err(err) => {
                                studio.notify(&err, true);
                                return;
                            }
                        };

                        let client = GeminiClient::new(token).with_quota_project(self.auth.quota_project());
                        self.busy = true;
                        self.status = "Finalizando cambios...".into();

                        let model = self.agent.model.clone();
                        let req = self.agent.build_request();

                        spawn(
                            move || client.generate_content(&model, &req),
                            |studio, _app, _models, result| {
                                studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                            },
                        );
                    } else {
                        let tool_res = {
                            let session = match studio.session.as_mut() {
                                Some(s) => s,
                                None => return,
                            };

                            match execute_tool(session, &action.tool_name, &action.args) {
                                Ok(r) => r,
                                Err(err) => {
                                    studio.notify(&format!("Error aplicando acción: {err}"), true);
                                    return;
                                }
                            }
                        };
                        studio.after_change();

                        // Mark last message as applied
                        if let Some(last) = self.agent.messages.iter_mut().rev().find(|m| m.proposed_action.is_some()) {
                            last.action_status = Some(ActionStatus::Applied);
                        }

                        self.agent.history.push(Content::function_response(&action.tool_name, tool_res));
                        self.auto_save_current_history();

                        let token = match self.auth.get_valid_token(&studio.host.dirs) {
                            Ok(t) => t,
                            Err(err) => {
                                studio.notify(&err, true);
                                return;
                            }
                        };

                        let client = GeminiClient::new(token).with_quota_project(self.auth.quota_project());
                        self.busy = true;
                        self.status = "Finalizando cambios...".into();

                        let model = self.agent.model.clone();
                        let req = self.agent.build_request();

                        spawn(
                            move || client.generate_content(&model, &req),
                            |studio, _app, _models, result| {
                                studio.handle(Msg::Assistant(AssistantMsg::ModelTurnDone(result)));
                            },
                        );
                    }
                }
            }

            AssistantMsg::RejectAction(_id) => {
                if let Some(action) = self.pending_action.take() {
                    let clean_name = action.tool_name.split(':').last().unwrap_or(&action.tool_name);
                    if clean_name == "batch" && action.args.get("multi_calls").is_some() {
                        let multi_calls = action.args.get("multi_calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        let mut responses = Vec::new();
                        for item in &multi_calls {
                            let name = item.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                            responses.push((
                                name.to_string(),
                                serde_json::json!({
                                    "status": "rejected",
                                    "message": "El usuario decidió no aplicar esta acción."
                                }),
                            ));
                        }
                        self.agent.history.push(Content::function_responses(responses));
                    } else {
                        let err_json = serde_json::json!({
                            "status": "rejected",
                            "message": "El usuario decidió no aplicar esta acción."
                        });
                        self.agent.history.push(Content::function_response(&action.tool_name, err_json));
                    }

                    if let Some(last) = self.agent.messages.iter_mut().rev().find(|m| m.proposed_action.is_some()) {
                        last.action_status = Some(ActionStatus::Rejected);
                    }
                    self.auto_save_current_history();
                }
            }

            AssistantMsg::Clear => {
                self.agent.clear();
                self.pending_action = None;
                self.busy = false;
                self.status.clear();
                self.auto_save_current_history();
            }
        }
    }

    /// Publishes current state to the Slint Editor global.
    pub fn publish(&self, app: &crate::ui::App) {
        let editor = app.global::<Editor>();

        editor.set_assistant_logged_in(self.auth.is_logged_in());
        editor.set_assistant_user_email(
            self.auth
                .user_email
                .as_deref()
                .or(if self.auth.api_key.is_some() {
                    Some("API Key Activa")
                } else {
                    None
                })
                .unwrap_or("")
                .into(),
        );
        editor.set_assistant_model(self.agent.model.as_str().into());
        editor.set_assistant_autonomous(self.agent.mode == ExecutionMode::Autonomous);
        editor.set_assistant_busy(self.busy);
        editor.set_assistant_status(self.status.as_str().into());

        let ui_messages: Vec<AssistantMessageData> = self
            .agent
            .messages
            .iter()
            .map(|m| AssistantMessageData {
                id: m.id.as_str().into(),
                sender: m.sender.as_str().into(),
                text: m.text.as_str().into(),
                has_action: m.proposed_action.is_some(),
                action_description: m
                    .proposed_action
                    .as_ref()
                    .map(|a| a.human_description.as_str())
                    .unwrap_or("")
                    .into(),
                action_status: match m.action_status {
                    Some(ActionStatus::Pending) => "pending".into(),
                    Some(ActionStatus::Applied) => "applied".into(),
                    Some(ActionStatus::Rejected) => "rejected".into(),
                    None => "".into(),
                },
                timestamp: if m.timestamp > 0 {
                    let secs = m.timestamp % 86400;
                    let hours = (secs / 3600) % 24;
                    let mins = (secs % 3600) / 60;
                    format!("{:02}:{:02}", hours, mins).into()
                } else {
                    "".into()
                },
            })
            .collect();

        editor.set_assistant_messages(ModelRc::new(VecModel::from(ui_messages)));
    }
}
