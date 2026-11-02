// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Tools that Gemini can call to inspect and modify Concat video projects.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use concat_project::commands::{ClipMove, ClipPatch, TrimEdge};
use concat_project::model::{AppliedFilter, TextStyle};
use concat_project::Command;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::client::{FunctionDeclaration, Tool};
use crate::Session;

/// A proposed action that can be previewed or executed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposedAction {
    pub tool_name: String,
    pub args: Value,
    pub human_description: String,
}

/// Returns the complete list of tools available to Gemini.
pub fn get_tools() -> Vec<Tool> {
    vec![Tool {
        function_declarations: vec![
            FunctionDeclaration {
                name: "get_project_summary".into(),
                description: "Returns current project dimensions, frame rate, active timeline, tracks, clips on the timeline with their start/duration/names, and media available in the bin.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            FunctionDeclaration {
                name: "split_clip".into(),
                description: "Splits a clip on the timeline into two pieces at a given playhead time in seconds.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id", "at_seconds"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to split (e.g. c1)" },
                        "at_seconds": { "type": "number", "description": "Timeline position in seconds where the cut should be made" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "delete_clip".into(),
                description: "Deletes a clip from the timeline.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to delete" },
                        "ripple": { "type": "boolean", "description": "If true, closes the gap by shifting subsequent clips left (magnetic track)" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "trim_clip".into(),
                description: "Trims either the start or end edge of a clip by a relative number of seconds.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id", "edge", "delta_seconds"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to trim" },
                        "edge": { "type": "string", "enum": ["start", "end"], "description": "'start' to trim head, 'end' to trim tail" },
                        "delta_seconds": { "type": "number", "description": "Signed delta in seconds: positive shortens head or lengthens tail" },
                        "ripple": { "type": "boolean", "description": "Whether to shift subsequent clips along with the trim" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "move_clip".into(),
                description: "Moves a clip to a new start time on the timeline and optionally to a different track.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id", "new_start_seconds"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to move" },
                        "new_start_seconds": { "type": "number", "description": "The new starting point in seconds (>= 0)" },
                        "track_id": { "type": "string", "description": "Optional destination track ID. Defaults to keeping the current track" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "add_text_clip".into(),
                description: "Adds a text or title clip to the timeline.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["text", "start_seconds"],
                    "properties": {
                        "text": { "type": "string", "description": "The text to display on screen" },
                        "start_seconds": { "type": "number", "description": "Timeline position in seconds where text begins" },
                        "duration_seconds": { "type": "number", "description": "Duration in seconds (default is 4.0)" },
                        "track_id": { "type": "string", "description": "Optional track ID. When omitted, places it on a free track above video" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "add_media_clip".into(),
                description: "Places a media file from the project bin onto a track on the timeline.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["media_id", "start_seconds"],
                    "properties": {
                        "media_id": { "type": "string", "description": "The ID of the media item from the bin (e.g. m1)" },
                        "start_seconds": { "type": "number", "description": "Timeline position in seconds" },
                        "duration_seconds": { "type": "number", "description": "Optional duration in seconds (especially for images, defaults to 5.0)" },
                        "track_id": { "type": "string", "description": "Optional target track ID. Defaults to the main track or lowest free track" },
                        "ripple": { "type": "boolean", "description": "Whether to push subsequent clips right if overlapping" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "add_multiple_clips".into(),
                description: "Places multiple media items (photos, videos, audio) from the project bin onto the timeline in sequence or at specified times. Highly recommended for batch adding photos, slideshows, or assembling montages.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "clips": {
                            "type": "array",
                            "description": "Array of clips to place. Can be objects {media_id, start_seconds?, duration_seconds?} or array of media_id strings.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "media_id": { "type": "string", "description": "The media ID from the bin (e.g. 'm1')" },
                                    "start_seconds": { "type": "number", "description": "Optional start time. If omitted, placed sequentially." },
                                    "duration_seconds": { "type": "number", "description": "Optional duration in seconds (defaults to 5.0 for images)." }
                                }
                            }
                        },
                        "media_ids": {
                            "type": "array",
                            "description": "Alternative simplified list of media IDs (e.g. ['m1', 'm2', 'm3']) to place in sequence.",
                            "items": { "type": "string" }
                        },
                        "all_images": {
                            "type": "boolean",
                            "description": "If true, automatically places all images from the project bin in order onto the timeline."
                        },
                        "start_from_seconds": {
                            "type": "number",
                            "description": "Start time on the timeline in seconds for the first clip (defaults to 0.0)."
                        },
                        "duration_per_clip": {
                            "type": "number",
                            "description": "Default duration in seconds for each image clip (defaults to 5.0)."
                        },
                        "gap_seconds": {
                            "type": "number",
                            "description": "Gap in seconds between consecutive clips (defaults to 0.0)."
                        },
                        "track_id": {
                            "type": "string",
                            "description": "Optional target track ID (e.g. 'T1'). If omitted, places on lowest free track or first video track."
                        }
                    }
                }),
            },
            FunctionDeclaration {
                name: "clear_timeline".into(),
                description: "Removes all clips from the active timeline to start with a blank timeline, or clears a specific track.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "track_id": { "type": "string", "description": "Optional track ID to clear. If omitted, removes all clips from all tracks on the active timeline." }
                    }
                }),
            },
            FunctionDeclaration {
                name: "apply_effect".into(),
                description: "Applies a visual filter or effect to a clip (e.g. sepia, blur, glow, concat.vignette, etc.).".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id", "effect_id"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip" },
                        "effect_id": { "type": "string", "description": "The identifier of the effect (e.g. 'sepia', 'blur', 'concat.vignette')" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "adjust_clip_properties".into(),
                description: "Adjusts properties of a clip such as volume, opacity, playback speed, or fade in/out.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to update" },
                        "volume": { "type": "number", "description": "Audio volume factor (1.0 = 100%, 0.0 = mute, >1.0 = boost)" },
                        "opacity": { "type": "number", "description": "Visual opacity between 0.0 (transparent) and 1.0 (opaque)" },
                        "speed": { "type": "number", "description": "Playback speed factor (e.g. 0.5 for slow motion, 2.0 for fast)" },
                        "fade_in": { "type": "number", "description": "Fade-in duration in seconds" },
                        "fade_out": { "type": "number", "description": "Fade-out duration in seconds" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "generate_image".into(),
                description: "Generates an image/photograph using AI from a text prompt, saves it to the project media, and places it as a clip onto the timeline.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": { "type": "string", "description": "Detailed text description of the image to generate" },
                        "start_seconds": { "type": "number", "description": "Timeline position in seconds where the clip will be placed (default: 0.0)" },
                        "duration_seconds": { "type": "number", "description": "Duration on timeline in seconds (default: 5.0)" },
                        "aspect_ratio": { "type": "string", "enum": ["16:9", "9:16", "1:1", "4:3"], "description": "Aspect ratio of the generated image (default: 16:9)" },
                        "track_id": { "type": "string", "description": "Target track ID. If omitted, places on the first video track" }
                    }
                }),
            },
            FunctionDeclaration {
                name: "edit_image_clip".into(),
                description: "Applies an AI generative transformation or stylistic edit to an existing image/photo clip on the timeline, generating a new styled version and placing or updating it on the timeline.".into(),
                parameters: json!({
                    "type": "object",
                    "required": ["clip_id", "prompt"],
                    "properties": {
                        "clip_id": { "type": "string", "description": "The ID of the clip to edit (e.g. c1)" },
                        "prompt": { "type": "string", "description": "The transformation, aesthetic style, or visual change to apply (e.g. 'cyberpunk neon style', 'oil painting', 'vintage 1970s warm film')" },
                        "replace": { "type": "boolean", "description": "If true, replaces the clip in place. If false, adds the new version on a free track above for comparison (default: true)" }
                    }
                }),
            },
        ],
    }]
}

/// Generates a friendly human description of what a proposed action will do.
pub fn describe_action(session: &Session, name: &str, args: &Value) -> String {
    let clean_name = name.split(':').last().unwrap_or(name);
    match clean_name {
        "get_project_summary" => "Inspeccionar el estado general del proyecto".into(),
        "split_clip" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let at = args.get("at_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let clip_name = find_clip_name(session, clip_id);
            format!("Dividir clip '{}' ({clip_id}) en el segundo {:.2}s", clip_name, at)
        }
        "delete_clip" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let ripple = args.get("ripple").and_then(|v| v.as_bool()).unwrap_or(false);
            let clip_name = find_clip_name(session, clip_id);
            if ripple {
                format!("Eliminar clip '{}' ({clip_id}) cerrando el espacio (ripple)", clip_name)
            } else {
                format!("Eliminar clip '{}' ({clip_id}) dejando el espacio vacío", clip_name)
            }
        }
        "trim_clip" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let edge = args.get("edge").and_then(|v| v.as_str()).unwrap_or("end");
            let delta = args.get("delta_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let clip_name = find_clip_name(session, clip_id);
            format!("Ajustar borde {edge} de '{}' ({clip_id}) en {:.2}s", clip_name, delta)
        }
        "move_clip" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let start = args.get("new_start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let clip_name = find_clip_name(session, clip_id);
            format!("Mover clip '{}' ({clip_id}) al segundo {:.2}s", clip_name, start)
        }
        "add_text_clip" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let start = args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            format!("Añadir texto \"{}\" en el segundo {:.2}s", text, start)
        }
        "add_media_clip" => {
            let media_id = args.get("media_id").and_then(|v| v.as_str()).unwrap_or("?");
            let start = args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let clip_name = session.project().media_by_id(media_id).map(|m| m.name.as_str()).unwrap_or(media_id);
            format!("Añadir clip '{}' en el segundo {:.2}s", clip_name, start)
        }
        "add_multiple_clips" => {
            let count = if args.get("all_images").and_then(|v| v.as_bool()).unwrap_or(false) {
                "todas las fotos del proyecto".to_string()
            } else if let Some(media_ids) = args.get("media_ids").and_then(|v| v.as_array()) {
                format!("{} fotos/clips", media_ids.len())
            } else if let Some(clips) = args.get("clips").and_then(|v| v.as_array()) {
                format!("{} fotos/clips", clips.len())
            } else {
                "varios medios".to_string()
            };
            let start = args.get("start_from_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let track = args.get("track_id").and_then(|v| v.as_str()).unwrap_or("la pista principal");
            format!("Colocar {count} en secuencia en {track} iniciando en {:.2}s", start)
        }
        "clear_timeline" => {
            if let Some(t) = args.get("track_id").and_then(|v| v.as_str()) {
                format!("Limpiar todos los clips de la pista {t}")
            } else {
                "Limpiar completamente la línea de tiempo (eliminar todos los clips)".to_string()
            }
        }
        "batch" => {
            if let Some(actions) = args.get("actions").and_then(|v| v.as_array()) {
                let lines: Vec<String> = actions
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let tool = a.get("tool_name").and_then(|v| v.as_str()).unwrap_or("?");
                        let a_args = a.get("args").unwrap_or(&serde_json::Value::Null);
                        format!("{}. {}", i + 1, describe_action(session, tool, a_args))
                    })
                    .collect();
                format!("Ejecutar lote de {} acciones:\n{}", actions.len(), lines.join("\n"))
            } else {
                "Ejecutar lote de acciones".to_string()
            }
        }
        "apply_effect" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let effect_id = args.get("effect_id").and_then(|v| v.as_str()).unwrap_or("?");
            let clip_name = find_clip_name(session, clip_id);
            format!("Aplicar efecto '{effect_id}' al clip '{}' ({clip_id})", clip_name)
        }
        "adjust_clip_properties" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let clip_name = find_clip_name(session, clip_id);
            format!("Ajustar propiedades del clip '{}' ({clip_id})", clip_name)
        }
        "generate_image" => {
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let start = args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            format!("Generar imagen con IA \"{prompt}\" e incrustarla en el segundo {:.2}s", start)
        }
        "edit_image_clip" => {
            let clip_id = args.get("clip_id").and_then(|v| v.as_str()).unwrap_or("?");
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let clip_name = find_clip_name(session, clip_id);
            format!("Transformar con IA la imagen '{}' ({clip_id}) con estilo: \"{prompt}\"", clip_name)
        }
        _ => format!("Ejecutar acción '{name}' con argumentos: {args}"),
    }
}

/// Executes a tool call directly on the project session.
pub fn execute_tool(session: &mut Session, name: &str, args: &Value) -> Result<Value, String> {
    let clean_name = name.split(':').last().unwrap_or(name);
    match clean_name {
        "get_project_summary" => {
            let project = session.project();
            let active = project.active();
            let mut tracks_summary = Vec::new();

            for track in &active.tracks {
                let clips_summary: Vec<Value> = active
                    .clips
                    .iter()
                    .filter(|clip| clip.track_id == track.id)
                    .map(|clip| {
                        json!({
                            "clip_id": clip.id,
                            "name": clip.name,
                            "start_seconds": clip.start,
                            "duration_seconds": clip.duration,
                            "speed": clip.speed,
                            "volume": clip.volume,
                            "opacity": clip.opacity,
                            "media_id": clip.media_id,
                            "text": clip.text.as_ref().map(|t| &t.content),
                            "effects": clip.video_effects.iter().map(|e| &e.id).collect::<Vec<_>>(),
                        })
                    })
                    .collect();

                tracks_summary.push(json!({
                    "track_id": track.id,
                    "visible": track.visible,
                    "muted": track.muted,
                    "clips": clips_summary,
                }));
            }

            let media_summary: Vec<Value> = project
                .media
                .iter()
                .map(|m| {
                    json!({
                        "media_id": m.id,
                        "name": m.name,
                        "duration_seconds": m.duration,
                        "kind": format!("{:?}", m.kind),
                        "width": m.width,
                        "height": m.height,
                    })
                })
                .collect();

            Ok(json!({
                "width": active.video.width,
                "height": active.video.height,
                "frame_rate": format!("{}/{}", active.video.rate_num, active.video.rate_den),
                "timeline_id": project.active_timeline_id,
                "tracks": tracks_summary,
                "media_bin": media_summary,
            }))
        }

        "split_clip" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let at = args
                .get("at_seconds")
                .and_then(|v| v.as_f64())
                .ok_or("Falta 'at_seconds'")?;

            let cmd = Command::SplitClips {
                clip_ids: vec![clip_id.to_string()],
                time: at,
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Clip {clip_id} dividido en {at}s"),
                "can_undo": view.can_undo
            }))
        }

        "delete_clip" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let ripple = args.get("ripple").and_then(|v| v.as_bool()).unwrap_or(false);

            let cmd = Command::RemoveClips {
                clip_ids: vec![clip_id.to_string()],
                ripple,
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Clip {clip_id} eliminado"),
                "can_undo": view.can_undo
            }))
        }

        "trim_clip" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let edge_str = args
                .get("edge")
                .and_then(|v| v.as_str())
                .unwrap_or("end");
            let edge = if edge_str.eq_ignore_ascii_case("start") {
                TrimEdge::Start
            } else {
                TrimEdge::End
            };
            let delta = args
                .get("delta_seconds")
                .and_then(|v| v.as_f64())
                .ok_or("Falta 'delta_seconds'")?;
            let ripple = args.get("ripple").and_then(|v| v.as_bool()).unwrap_or(false);

            let cmd = Command::TrimClip {
                clip_id: clip_id.to_string(),
                edge,
                delta,
                ripple,
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Borde {edge_str} de {clip_id} recortado por {delta}s"),
                "can_undo": view.can_undo
            }))
        }

        "move_clip" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let new_start = args
                .get("new_start_seconds")
                .and_then(|v| v.as_f64())
                .ok_or("Falta 'new_start_seconds'")?;

            // Find current track if none provided
            let track_id = if let Some(t) = args.get("track_id").and_then(|v| v.as_str()) {
                t.to_string()
            } else {
                find_clip_track(session, clip_id).ok_or("Clip no encontrado para determinar su pista actual")?
            };

            let cmd = Command::MoveClips {
                moves: vec![ClipMove {
                    clip_id: clip_id.to_string(),
                    start: new_start,
                    track_id,
                }],
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Clip {clip_id} movido a {new_start}s"),
                "can_undo": view.can_undo
            }))
        }

        "add_text_clip" => {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'text'")?;
            let start = args
                .get("start_seconds")
                .and_then(|v| v.as_f64())
                .ok_or("Falta 'start_seconds'")?;
            let duration = args.get("duration_seconds").and_then(|v| v.as_f64());
            let track_id = args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string());

            let mut style = TextStyle::default();
            style.content = text.to_string();

            let cmd = Command::AddTextClip {
                track_id,
                above: true,
                start,
                style: Some(style),
                duration,
                offset_y: None,
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Texto añadido en {start}s"),
                "created_id": view.created_id,
                "can_undo": view.can_undo
            }))
        }

        "add_media_clip" => {
            let media_id = args
                .get("media_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'media_id'")?;
            let start = args
                .get("start_seconds")
                .and_then(|v| v.as_f64())
                .ok_or("Falta 'start_seconds'")?;
            let track_id = args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| {
                    session
                        .project()
                        .active()
                        .tracks
                        .first()
                        .map(|t| t.id.clone())
                        .unwrap_or_else(|| "T1".to_string())
                });
            let ripple = args.get("ripple").and_then(|v| v.as_bool()).unwrap_or(false);

            let cmd = Command::AddClip {
                media_id: media_id.to_string(),
                track_id: track_id.clone(),
                start,
                ripple,
            };
            let view = session.apply(cmd)?;

            if let Some(dur) = args.get("duration_seconds").and_then(|v| v.as_f64()) {
                if let Some(cid) = &view.created_id {
                    let is_image = session.project().media_by_id(media_id).map(|m| m.kind == concat_project::model::MediaKind::Image).unwrap_or(false);
                    if is_image && (dur - 5.0).abs() > 0.05 {
                        let delta = dur - 5.0;
                        let _ = session.apply(Command::TrimClip {
                            clip_id: cid.clone(),
                            edge: TrimEdge::End,
                            delta,
                            ripple: false,
                        });
                    }
                }
            }

            Ok(json!({
                "status": "success",
                "message": format!("Medio {media_id} colocado en la pista {track_id}"),
                "created_id": view.created_id,
                "can_undo": view.can_undo
            }))
        }

        "add_multiple_clips" => {
            let mut items_to_add: Vec<(String, Option<f64>, Option<f64>)> = Vec::new();

            if args.get("all_images").and_then(|v| v.as_bool()).unwrap_or(false) {
                for m in &session.project().media {
                    if m.kind == concat_project::model::MediaKind::Image {
                        items_to_add.push((m.id.clone(), None, None));
                    }
                }
            } else if let Some(media_ids) = args.get("media_ids").and_then(|v| v.as_array()) {
                for id_val in media_ids {
                    if let Some(id_str) = id_val.as_str() {
                        items_to_add.push((id_str.to_string(), None, None));
                    }
                }
            } else if let Some(clips) = args.get("clips").and_then(|v| v.as_array()) {
                for c in clips {
                    if let Some(id_str) = c.as_str() {
                        items_to_add.push((id_str.to_string(), None, None));
                    } else if let Some(obj) = c.as_object() {
                        let mid = obj.get("media_id").and_then(|v| v.as_str()).unwrap_or("");
                        if !mid.is_empty() {
                            let st = obj.get("start_seconds").and_then(|v| v.as_f64());
                            let dur = obj.get("duration_seconds").and_then(|v| v.as_f64());
                            items_to_add.push((mid.to_string(), st, dur));
                        }
                    }
                }
            }

            if items_to_add.is_empty() {
                return Err("No se especificaron clips ni 'media_ids' para agregar por lotes".to_string());
            }

            let default_dur = args.get("duration_per_clip").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let gap = args.get("gap_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mut cursor = args.get("start_from_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let target_track = args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string());

            let track_id = if let Some(tid) = target_track {
                tid
            } else {
                session
                    .project()
                    .active()
                    .tracks
                    .first()
                    .map(|t| t.id.clone())
                    .unwrap_or_else(|| "T1".to_string())
            };

            let mut added_clip_ids = Vec::new();

            for (media_id, explicit_start, explicit_dur) in items_to_add {
                let media_item = session.project().media_by_id(&media_id).cloned();
                let (is_image, natural_dur) = match &media_item {
                    Some(m) => (m.kind == concat_project::model::MediaKind::Image, m.duration.unwrap_or(5.0)),
                    None => (false, 5.0),
                };

                let start = explicit_start.unwrap_or(cursor);
                let duration = explicit_dur.unwrap_or_else(|| if is_image { default_dur } else { natural_dur });

                let view = session.apply(Command::AddClip {
                    media_id: media_id.clone(),
                    track_id: track_id.clone(),
                    start,
                    ripple: false,
                })?;

                if let Some(cid) = &view.created_id {
                    if is_image && (duration - 5.0).abs() > 0.05 {
                        let delta = duration - 5.0;
                        let _ = session.apply(Command::TrimClip {
                            clip_id: cid.clone(),
                            edge: TrimEdge::End,
                            delta,
                            ripple: false,
                        });
                    }
                    added_clip_ids.push(cid.clone());
                }

                cursor = start + duration + gap;
            }

            Ok(json!({
                "status": "success",
                "message": format!("Se colocaron {} clips en la pista {}", added_clip_ids.len(), track_id),
                "clips_added": added_clip_ids.len(),
                "created_clip_ids": added_clip_ids,
                "track_id": track_id,
                "end_time_seconds": cursor
            }))
        }

        "clear_timeline" => {
            let target_track = args.get("track_id").and_then(|v| v.as_str());
            let clip_ids: Vec<String> = session
                .project()
                .active()
                .clips
                .iter()
                .filter(|c| match target_track {
                    Some(t) => c.track_id == t,
                    None => true,
                })
                .map(|c| c.id.clone())
                .collect();

            let count = clip_ids.len();
            if !clip_ids.is_empty() {
                session.apply(Command::RemoveClips {
                    clip_ids,
                    ripple: false,
                })?;
            }

            Ok(json!({
                "status": "success",
                "message": format!("Se eliminaron {count} clips de la línea de tiempo"),
                "cleared_clips_count": count
            }))
        }

        "batch" => {
            let actions = args
                .get("actions")
                .and_then(|v| v.as_array())
                .ok_or("Falta 'actions' en comando batch")?;

            let mut results = Vec::new();
            for a in actions {
                let tool = a.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let a_args = a.get("args").unwrap_or(&serde_json::Value::Null);
                let res = execute_tool(session, tool, a_args)?;
                results.push(res);
            }

            Ok(json!({
                "status": "success",
                "message": format!("Lote de {} acciones ejecutado con éxito", actions.len()),
                "results": results
            }))
        }

        "apply_effect" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let effect_id = args
                .get("effect_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'effect_id'")?;

            let mut patch = ClipPatch::default();
            patch.video_effects = Some(vec![AppliedFilter::new(effect_id)]);

            let cmd = Command::UpdateClip {
                clip_id: clip_id.to_string(),
                patch,
            };
            let view = session.apply(cmd)?;
            Ok(json!({
                "status": "success",
                "message": format!("Efecto {effect_id} aplicado al clip {clip_id}"),
                "can_undo": view.can_undo
            }))
        }

        "adjust_clip_properties" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;

            let mut patch = ClipPatch::default();
            if let Some(vol) = args.get("volume").and_then(|v| v.as_f64()) {
                patch.volume = Some(vol);
            }
            if let Some(op) = args.get("opacity").and_then(|v| v.as_f64()) {
                patch.opacity = Some(op);
            }
            if let Some(fi) = args.get("fade_in").and_then(|v| v.as_f64()) {
                patch.fade_in = Some(fi);
            }
            if let Some(fo) = args.get("fade_out").and_then(|v| v.as_f64()) {
                patch.fade_out = Some(fo);
            }

            let view = session.apply(Command::UpdateClip {
                clip_id: clip_id.to_string(),
                patch,
            })?;

            if let Some(speed) = args.get("speed").and_then(|v| v.as_f64()) {
                session.apply(Command::SetClipSpeed {
                    clip_id: clip_id.to_string(),
                    speed,
                })?;
            }

            Ok(json!({
                "status": "success",
                "message": format!("Propiedades actualizadas para {clip_id}"),
                "can_undo": view.can_undo
            }))
        }

        "generate_image" => {
            let prompt = args
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'prompt' con la descripción de la imagen a generar")?;
            let start = args.get("start_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let duration = args.get("duration_seconds").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let aspect_ratio = args.get("aspect_ratio").and_then(|v| v.as_str()).unwrap_or("16:9");
            let target_track = args.get("track_id").and_then(|v| v.as_str()).map(|s| s.to_string());

            let (width, height) = match aspect_ratio {
                "9:16" => (720, 1280),
                "1:1" => (1024, 1024),
                "4:3" => (1024, 768),
                _ => (1280, 720),
            };

            let bytes = fetch_ai_image(prompt, width, height)?;
            apply_generated_image(session, &bytes, prompt, start, duration, target_track)
        }

        "edit_image_clip" => {
            let clip_id = args
                .get("clip_id")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'clip_id'")?;
            let prompt = args
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'prompt' con la modificación o estilo deseado")?;
            let replace = args.get("replace").and_then(|v| v.as_bool()).unwrap_or(true);

            let active = session.project().active();
            let clip = active
                .clips
                .iter()
                .find(|c| c.id == clip_id)
                .cloned()
                .ok_or_else(|| format!("No se encontró el clip {clip_id}"))?;

            let old_start = clip.start;
            let old_duration = clip.duration;
            let old_track = clip.track_id.clone();
            let clip_name = clip.name.clone();

            let full_prompt = format!("{}, high quality cinematography: {}", clip_name, prompt);
            let bytes = fetch_ai_image(&full_prompt, 1280, 720)?;
            let dir = get_ai_media_dir(session);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let file_path = dir.join(format!("ai_edit_{}_{timestamp}.jpg", clip_id));
            std::fs::write(&file_path, &bytes)
                .map_err(|e| format!("No se pudo guardar la imagen editada: {e}"))?;

            let path_str = file_path.to_string_lossy().into_owned();
            let summary = crate::media::probe(&path_str)?;
            session.apply(Command::AddMedia {
                item: summary.to_new_media(),
            })?;

            let new_media_id = session
                .project()
                .media
                .iter()
                .find(|m| m.path == path_str)
                .map(|m| m.id.clone())
                .ok_or("No se encontró el nuevo medio tras importarlo")?;

            let final_clip_id = if replace {
                session.apply(Command::RemoveClips {
                    clip_ids: vec![clip_id.to_string()],
                    ripple: false,
                })?;
                let view = session.apply(Command::AddClip {
                    media_id: new_media_id.clone(),
                    track_id: old_track.clone(),
                    start: old_start,
                    ripple: false,
                })?;
                if (old_duration - 5.0).abs() > 0.05 {
                    if let Some(cid) = &view.created_id {
                        let delta = old_duration - 5.0;
                        let _ = session.apply(Command::TrimClip {
                            clip_id: cid.clone(),
                            edge: TrimEdge::End,
                            delta,
                            ripple: false,
                        });
                    }
                }
                view.created_id
            } else {
                let view = session.apply(Command::AddClipAtFirstFree {
                    media_id: new_media_id.clone(),
                    start: old_start,
                })?;
                view.created_id
            };

            Ok(json!({
                "status": "success",
                "message": format!("Clip {} transformado con estilo '{}' y colocado en la línea de tiempo", clip_id, prompt),
                "new_media_id": new_media_id,
                "clip_id": final_clip_id,
                "file_path": path_str,
            }))
        }

        other => Err(format!("Herramienta desconocida: {other}")),
    }
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

/// Fetches an AI-generated image from Pollinations (trying flux first, then turbo fallback) in a background thread.
pub fn fetch_ai_image(prompt: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let trimmed = prompt.trim();
    let clean_prompt = if trimmed.to_lowercase().contains("no people") || trimmed.to_lowercase().contains("clean background") {
        trimmed.to_string()
    } else {
        format!(
            "{}, photorealistic, empty environment without people, clean background, no watermark, no logo, no text",
            trimmed
        )
    };
    let encoded = url_encode(&clean_prompt);

    for model in ["flux", "turbo"] {
        let url = format!(
            "https://image.pollinations.ai/prompt/{}?width={}&height={}&model={}&nologo=true&private=true&enhance=false",
            encoded, width, height, model
        );
        log::info!("fetch_ai_image: connecting to pollinations with model {model} (prompt len: {})...", clean_prompt.len());

        let req = ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(65));

        match req.call() {
            Ok(resp) => {
                let mut bytes = Vec::new();
                if resp.into_reader().read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                    log::info!("fetch_ai_image: successfully received {} bytes from model {model}", bytes.len());
                    return Ok(bytes);
                }
            }
            Err(e) => {
                log::warn!("fetch_ai_image: model {model} failed: {e}, attempting next fallback model...");
            }
        }
    }

    Err("No se pudo conectar con el servicio de IA de imágenes o la solicitud expiró".into())
}

/// Applies a downloaded AI image to the session timeline.
pub fn apply_generated_image(
    session: &mut Session,
    bytes: &[u8],
    _prompt: &str,
    start: f64,
    duration: f64,
    target_track: Option<String>,
) -> Result<Value, String> {
    let dir = get_ai_media_dir(session);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_path = dir.join(format!("ai_img_{timestamp}.jpg"));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&file_path)
            .map_err(|e| format!("No se pudo crear archivo de imagen: {e}"))?;
        file.write_all(bytes)
            .map_err(|e| format!("No se pudo escribir archivo de imagen: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("No se pudo sincronizar archivo de imagen: {e}"))?;
    }

    let path_str = file_path.to_string_lossy().into_owned();
    let summary = crate::media::probe(&path_str)?;
    session.apply(Command::AddMedia {
        item: summary.to_new_media(),
    })?;

    let media_id = session
        .project()
        .media
        .iter()
        .find(|m| m.path == path_str)
        .map(|m| m.id.clone())
        .ok_or("No se encontró el medio tras importarlo")?;

    let view = if let Some(track_id) = target_track {
        session.apply(Command::AddClip {
            media_id: media_id.clone(),
            track_id,
            start,
            ripple: false,
        })?
    } else {
        session.apply(Command::AddClipAtFirstFree {
            media_id: media_id.clone(),
            start,
        })?
    };

    let created_clip_id = view.created_id.clone();

    if (duration - 5.0).abs() > 0.05 {
        if let Some(cid) = &created_clip_id {
            let delta = duration - 5.0;
            let _ = session.apply(Command::TrimClip {
                clip_id: cid.clone(),
                edge: TrimEdge::End,
                delta,
                ripple: false,
            });
        }
    }

    Ok(json!({
        "status": "success",
        "message": format!("Imagen generada con IA e incrustada en la línea de tiempo en el segundo {:.2}s", start),
        "created_clip_id": created_clip_id,
        "media_id": media_id,
        "file_path": path_str,
        "duration_seconds": duration,
        "can_undo": view.can_undo
    }))
}

fn get_ai_media_dir(session: &Session) -> PathBuf {
    let proj_path = session.path();
    let dir = if !proj_path.is_empty() && Path::new(proj_path).exists() {
        Path::new(proj_path).join("ai_generated")
    } else if let Ok(dirs) = crate::dirs::AppDirs::locate() {
        dirs.data.join("ai_generated")
    } else {
        std::env::temp_dir().join("concat_ai_media")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn find_clip_name(session: &Session, clip_id: &str) -> String {
    for clip in &session.project().active().clips {
        if clip.id == clip_id {
            return clip.name.clone();
        }
    }
    clip_id.to_string()
}

fn find_clip_track(session: &Session, clip_id: &str) -> Option<String> {
    for clip in &session.project().active().clips {
        if clip.id == clip_id {
            return Some(clip.track_id.clone());
        }
    }
    None
}
