// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Conversational agent for Gemini managing context and execution loops.

use serde::{Deserialize, Serialize};

use super::client::{
    Content, GeminiClient, GenerateContentRequest, SystemInstruction,
    DEFAULT_MODEL,
};
use super::tools::{describe_action, execute_tool, get_tools, ProposedAction};
use crate::Session;

/// Execution mode for Gemini assistant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExecutionMode {
    /// Applies actions automatically as the AI decides.
    #[default]
    Autonomous,
    /// Shows interactive action cards that the user must approve before execution.
    Copilot,
}

/// A chat message displayed in the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String, // "user", "gemini", "system"
    pub text: String,
    pub proposed_action: Option<ProposedAction>,
    pub action_status: Option<ActionStatus>, // "pending", "applied", "rejected"
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    Pending,
    Applied,
    Rejected,
}

/// Agent that coordinates conversation with Gemini and executes tools.
pub struct GeminiAgent {
    pub model: String,
    pub mode: ExecutionMode,
    pub history: Vec<Content>,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: String,
}

/// Master system prompt defining Concat's dedicated Video Editing Specialist Gem.
pub const SPECIALIST_SYSTEM_PROMPT: &str = r#"Eres "Concat AI Master Editor", la Gema oficial y especialista en montaje y edición de video de Concat.

TU ROL Y EXPERIENCIA:
Operas como un montajista y editor de video profesional galardonado. Dominas la teoría del montaje cinematográfico (ritmo visual y sonoro, Walter Murch, regla de 6, cortes en el movimiento, match cuts, jump cuts, L-cuts, J-cuts, elipsis narrativa, ritmo musical y dinámicas modernas para YouTube, Reels, TikTok y producciones cinematográficas).

ENTORNO DE CONCAT:
Concat es un editor no lineal (NLE) local de alta precisión por fotograma. La línea de tiempo está organizada en pistas (tracks) y clips con marcas de tiempo exactas en segundos (start_seconds, duration_seconds), volumen, opacidad, velocidad y efectos de video.
Tienes acceso directo y control de las herramientas del proyecto para inspeccionar y realizar modificaciones quirúrgicas.

TUS HERRAMIENTAS DISPONIBLES:
1. `get_project_summary`: Inspecciona resolución, FPS, pistas, clips existentes (ID, nombre, inicio, duración, volumen, velocidad, opacidad, texto, efectos) y medios en el bin.
2. `split_clip`: Divide un clip en dos en un segundo específico (clip_id, at_seconds).
3. `delete_clip`: Elimina un clip de la línea de tiempo (clip_id, ripple: bool para cerrar o dejar el espacio).
4. `trim_clip`: Recorta el inicio o final de un clip en segundos relativos (clip_id, edge: "start"|"end", delta_seconds, ripple: bool).
5. `move_clip`: Mueve un clip a un nuevo tiempo de inicio o a otra pista (clip_id, new_start_seconds, track_id).
6. `add_text_clip`: Añade un título o subtítulo en pantalla (text, start_seconds, duration_seconds, track_id).
7. `add_media_clip`: Inserta un recurso individual del bin a la línea de tiempo (media_id, start_seconds, duration_seconds, track_id, ripple: bool).
8. `add_multiple_clips`: Coloca MÚLTIPLES fotos o clips del bin en la línea de tiempo en un solo paso. Muy recomendado para procesar lotes, secuencias cronológicas, carruseles y montajes de fotos (clips: [{media_id, start_seconds?, duration_seconds?}], media_ids: ["m1", "m2", ...], all_images: bool, start_from_seconds: number, duration_per_clip: number, gap_seconds: number, track_id: string).
9. `clear_timeline`: Limpia la línea de tiempo activa eliminando todos los clips (o los de una pista específica si se indica track_id).
10. `apply_effect`: Aplica filtros visuales y paquetes de etalonaje cinematográfico (clip_id, effect_id). Concat cuenta con más de 120 paquetes de efectos integrados de calidad de estudio:
   - Estilísticos: 'concat.cyberpunk', 'concat.noir', 'concat.golden-hour', 'concat.teal-orange', 'concat.vintage', 'concat.vhs', 'concat.retro-70s', 'concat.polaroid', 'concat.super-8', 'concat.vaporwave'
   - Corrección y Óptica: 'concat.film-grain', 'concat.vignette', 'concat.gaussian-blur', 'concat.zoom-blur', 'concat.bloom-pulse', 'concat.glow', 'concat.chromatic-aberration', 'concat.color-wheels', 'concat.contrast', 'concat.saturation', 'concat.white-balance'
11. `adjust_clip_properties`: Modifica volumen (1.0 = 100%), opacidad (0.0-1.0), velocidad (speed, ej: 0.5 para slow-motion, 2.0 para acelerar), fade_in y fade_out en segundos.
12. `generate_image`: Genera una imagen fotorrealista o artística con IA a partir de un texto (prompt, start_seconds, duration_seconds, aspect_ratio: "16:9"|"9:16"|"1:1"|"4:3", track_id) y la incrusta directamente en la línea de tiempo.
13. `edit_image_clip`: Transforma o reinterpreta con IA una foto existente de la línea de tiempo (clip_id, prompt con el nuevo estilo/modificación, replace: bool) y la sitúa en la línea de tiempo.

REGLAS DE ORO DE EDICIÓN:
1. INSPECCIÓN OBLIGATORIA: Si el usuario te pide editar, recortar, analizar o modificar la línea de tiempo y no tienes el resumen reciente del proyecto, llama a `get_project_summary` primero para obtener los IDs reales (ej. "c1", "c2") y las marcas de tiempo exactas. NUNCA inventes IDs de clips o medios.
2. OPERACIONES POR LOTES (BATCH):
   - Cuando el usuario pida añadir varias o todas las fotos/clips importados a la línea de tiempo, o armar una secuencia con las fotos disponibles, NUNCA uses múltiples llamadas individuales de `add_media_clip`. Usa SIEMPRE `add_multiple_clips` con la lista de IDs o con `all_images: true`.
   - Si el usuario pide "limpia y agrega" o "reordena todo desde cero", puedes llamar a `clear_timeline` y luego a `add_multiple_clips`.
3. PRECISIÓN TEMPORAL: Todos los tiempos son en segundos (ej: 4.5s, 18.25s). Sé exacto.
4. RITMO Y DINAMISMO:
   - Si el usuario pide "hazlo más dinámico" o "elimina partes aburridas/silencios", busca pausas o tomas innecesariamente largas, divide y recorta con `ripple: true` para mantener un flujo ágil.
   - Si pide "corta los primeros X segundos", recorta el cabezal del clip con ripple.
   - Para títulos o subtítulos, colócalos en una pista superior y dales una duración legible acorde al texto (aprox. 1 palabra cada 0.3s, mínimo 2s por frase).
5. GENERACIÓN Y EDICIÓN DE FOTOS:
   - Cuando el usuario pida imágenes, ilustraciones, portadas, fondos o b-roll generados, crea prompts descriptivos cinematográficos (iluminación, composición, lente, atmósfera) y usa `generate_image`.
   - Cuando pida modificar o cambiar el estilo de una foto/clip existente, usa `edit_image_clip` para transformación generativa o `apply_effect` si busca un ajuste de color o filtro específico.
6. COMUNICACIÓN PROFESIONAL:
   - Sé conciso, proactivo y resolutivo.
   - Responde SIEMPRE en español con terminología clara de editor de video.
   - Explica brevemente la decisión técnica tomada (ej: "Coloqué 10 fotos en secuencia en la pista T1 con 5 segundos por imagen").
"#;

impl GeminiAgent {
    pub fn new() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            mode: ExecutionMode::Autonomous,
            history: Vec::new(),
            messages: Vec::new(),
            system_prompt: SPECIALIST_SYSTEM_PROMPT.to_string(),
        }
    }

    /// Clears conversation history.
    pub fn clear(&mut self) {
        self.history.clear();
        self.messages.clear();
    }

    /// Builds a GenerateContentRequest payload with the current history and system prompt.
    pub fn build_request(&self) -> GenerateContentRequest {
        GenerateContentRequest {
            contents: self.history.clone(),
            system_instruction: Some(SystemInstruction::new(&self.system_prompt)),
            tools: Some(get_tools()),
            generation_config: None,
        }
    }

    /// Records a user message into the UI messages and Gemini history.
    pub fn record_user_message(&mut self, text: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let user_msg_id = format!("msg_{}", self.messages.len() + 1);
        self.messages.push(ChatMessage {
            id: user_msg_id,
            sender: "user".into(),
            text: text.to_string(),
            proposed_action: None,
            action_status: None,
            timestamp,
        });
        self.history.push(Content::user(text));
    }

    /// Records an applied action into UI chat messages.
    pub fn record_action_applied(&mut self, action: ProposedAction) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.messages.push(ChatMessage {
            id: format!("action_{}", self.messages.len() + 1),
            sender: "gemini".into(),
            text: format!("Acción ejecutada: {}", action.human_description),
            proposed_action: Some(action),
            action_status: Some(ActionStatus::Applied),
            timestamp,
        });
    }

    /// Records a normal model text reply into UI chat messages.
    pub fn record_model_text(&mut self, text: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.messages.push(ChatMessage {
            id: format!("msg_{}", self.messages.len() + 1),
            sender: "gemini".into(),
            text: text.to_string(),
            proposed_action: None,
            action_status: None,
            timestamp,
        });
    }

    /// Handles a user message and returns any immediate response or proposed actions.
    pub fn ask(
        &mut self,
        client: &GeminiClient,
        session: &mut Session,
        user_text: &str,
    ) -> Result<AgentStepResult, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Add to UI messages
        let user_msg_id = format!("msg_{}", self.messages.len() + 1);
        self.messages.push(ChatMessage {
            id: user_msg_id,
            sender: "user".into(),
            text: user_text.to_string(),
            proposed_action: None,
            action_status: None,
            timestamp,
        });

        // Add to Gemini Content history
        self.history.push(Content::user(user_text));

        self.step(client, session)
    }

    /// Internal execution step (model turn -> tool call or response).
    pub fn step(
        &mut self,
        client: &GeminiClient,
        session: &mut Session,
    ) -> Result<AgentStepResult, String> {
        let req = GenerateContentRequest {
            contents: self.history.clone(),
            system_instruction: Some(SystemInstruction::new(&self.system_prompt)),
            tools: Some(get_tools()),
            generation_config: None,
        };

        let response = client.generate_content(&self.model, &req)?;
        let candidate = response
            .candidates
            .and_then(|c| c.into_iter().next())
            .ok_or_else(|| "Gemini no devolvió ninguna respuesta".to_string())?;

        let Some(content) = candidate.content else {
            return Err("Gemini devolvió un contenido vacío".to_string());
        };

        // Record model turn in conversation history if it contains valid parts
        if content.parts.iter().any(|p| p.is_valid()) {
            self.history.push(content.clone());
        }

        // Check for function calls
        for part in &content.parts {
            if let Some(call) = &part.function_call {
                let description = describe_action(session, &call.name, &call.args);
                let proposed = ProposedAction {
                    tool_name: call.name.clone(),
                    args: call.args.clone(),
                    human_description: description.clone(),
                };

                let clean_name = call.name.split(':').last().unwrap_or(&call.name);
                let is_read_only = clean_name == "get_project_summary";

                if is_read_only {
                    let result = execute_tool(session, &call.name, &call.args)?;
                    self.history.push(Content::function_response(&call.name, result));
                    return self.step(client, session);
                }

                match self.mode {
                    ExecutionMode::Autonomous => {
                        // In autonomous mode: execute immediately and continue loop
                        let result = execute_tool(session, &call.name, &call.args)?;
                        self.history.push(Content::function_response(&call.name, result));

                        // Add action info to UI
                        self.messages.push(ChatMessage {
                            id: format!("action_{}", self.messages.len() + 1),
                            sender: "gemini".into(),
                            text: format!("Acción ejecutada: {description}"),
                            proposed_action: Some(proposed),
                            action_status: Some(ActionStatus::Applied),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        });

                        // Continue the turn to get the next response from Gemini
                        return self.step(client, session);
                    }

                    ExecutionMode::Copilot => {
                        // In Copilot mode: pause and yield proposed action for user approval
                        let msg_id = format!("prop_{}", self.messages.len() + 1);
                        let msg = ChatMessage {
                            id: msg_id.clone(),
                            sender: "gemini".into(),
                            text: format!("He preparado la siguiente acción: {description}"),
                            proposed_action: Some(proposed.clone()),
                            action_status: Some(ActionStatus::Pending),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        };
                        self.messages.push(msg);

                        return Ok(AgentStepResult::ActionProposed(proposed));
                    }
                }
            }
        }

        // Check for normal text response
        let mut full_text = String::new();
        for part in &content.parts {
            if let Some(txt) = &part.text {
                full_text.push_str(txt);
            }
        }

        if !full_text.is_empty() {
            let msg_id = format!("msg_{}", self.messages.len() + 1);
            self.messages.push(ChatMessage {
                id: msg_id,
                sender: "gemini".into(),
                text: full_text.clone(),
                proposed_action: None,
                action_status: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            return Ok(AgentStepResult::Message(full_text));
        }

        Ok(AgentStepResult::Done)
    }

    /// User approved a proposed action in Copilot mode.
    pub fn approve_action(
        &mut self,
        client: &GeminiClient,
        session: &mut Session,
        action: &ProposedAction,
    ) -> Result<AgentStepResult, String> {
        let result = execute_tool(session, &action.tool_name, &action.args)?;
        self.history.push(Content::function_response(&action.tool_name, result));

        // Mark last message as applied
        if let Some(last) = self.messages.iter_mut().rev().find(|m| m.proposed_action.is_some()) {
            last.action_status = Some(ActionStatus::Applied);
        }

        // Continue agent conversation
        self.step(client, session)
    }

    /// User rejected a proposed action in Copilot mode.
    pub fn reject_action(
        &mut self,
        _client: &GeminiClient,
        action: &ProposedAction,
    ) -> Result<(), String> {
        let err_json = serde_json::json!({
            "status": "rejected",
            "message": "El usuario decidió no aplicar esta acción."
        });
        self.history.push(Content::function_response(&action.tool_name, err_json));

        if let Some(last) = self.messages.iter_mut().rev().find(|m| m.proposed_action.is_some()) {
            last.action_status = Some(ActionStatus::Rejected);
        }

        Ok(())
    }
}

pub enum AgentStepResult {
    Message(String),
    ActionProposed(ProposedAction),
    Done,
}
