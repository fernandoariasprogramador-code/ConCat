// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Generative AI and Cinematic Blanking Image Expansion pane controller:
//! expands vertical photographs to 16:9 horizontally by creating professional
//! ambient bokeh backgrounds, mirrored fills, or AI outpainting.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use concat_host::gemini::client::{Content, GeminiClient, GenerateContentRequest, Part, SystemInstruction};
use concat_host::gemini::tools::fetch_ai_image;
use concat_project::commands::ClipPatch;
use concat_project::model::AppliedFilter;
use concat_project::Command;

use crate::studio::Studio;
use crate::ui::AiExpandData;

/// Default outpainting directive prompt requested by the user.
pub const DEFAULT_OUTPAINTING_PROMPT: &str = r#"Actúa como un Director de Fotografía, Especialista en VFX de Outpainting y Analista de Visión Computacional de precisión milimétrica.

Tu objetivo es analizar la imagen vertical proporcionada (que se ubicará exactamente en el eje central) y redactar una especificación técnica y visual hiperdetallada para expandir el lienzo horizontalmente hasta alcanzar una relación de aspecto panorámica de 16:9, manteniendo una continuidad geométrica, lumínica y óptica perfecta en los nuevos paneles izquierdo y derecho.

Realiza un escaneo de los bordes verticales de la imagen original (los últimos 15 píxeles del margen izquierdo y del margen derecho, de arriba hacia abajo) y entrega tu respuesta siguiendo estrictamente esta estructura:

### 1. AUDITORÍA ÓPTICA Y TÉCNICA DEL CENTRO (IMAGEN BASE)
* **Geometría y Perspectiva:** Altura exacta de la línea del horizonte (medida en porcentaje de 0% arriba a 100% abajo), ubicación de los puntos de fuga y ángulo de inclinación de las líneas principales.
* **Parámetros de Cámara:** Distancia focal estimada (ej. 24mm, 50mm, 85mm), apertura/profundidad de campo (nivel de desenfoque o bokeh en primer plano, plano medio y fondo) y distorsión de lente en los extremos.
* **Iluminación y Colorimetría:** Dirección exacta de la fuente de luz principal y secundaria (en grados de reloj), dureza/suavidad de las sombras, temperatura de color (en grados Kelvin), grano/ruido fotográfico y paleta de códigos HEX dominantes en ambos bordes.

### 2. MAPEO MILIMÉTRICO DE COSTURA (SEAM MATCHING)
Divide los bordes verticales izquierdo y derecho en 3 cuadrantes de altura:
* **Zona Superior (0% - 33% de altura):** Qué texturas, colores y líneas se cortan exactamente en el borde izquierdo y en el derecho (ej. cielo, techo, follaje, arquitectura).
* **Zona Media (34% - 66% de altura):** Qué objetos, estructuras o fondos quedan truncados en ambos márgenes y cómo deben proyectarse hacia afuera sin deformarse.
* **Zona Inferior (67% - 100% de altura):** Qué superficies de suelo, sombras proyectadas o elementos en primer plano se cortan en la base izquierda y derecha.

### 3. DESCRIPCIÓN HIPERDETALLADA DEL LATERAL IZQUIERDO (EXPANSIÓN IZQUIERDA)
Describe con precisión milimétrica qué debe generarse desde el borde izquierdo de la foto original hasta el nuevo límite extremo izquierdo del lienzo 16:9:
* **Continuación inmediata (zona de transición de los primeros milímetros):** Cómo empatan las líneas, texturas y sombras que cruzan el borde izquierdo.
* **Desarrollo del plano (Fondo, Plano Medio y Primer Plano):** Qué elementos coherentes con el contexto aparecen progresivamente hacia la izquierda sin robar el protagonismo al sujeto central.
* **Comportamiento de la luz:** Cómo decae o rebota la iluminación en este sector izquierdo.

### 4. DESCRIPCIÓN HIPERDETALLADA DEL LATERAL DERECHO (EXPANSIÓN DERECHA)
Describe con precisión milimétrica qué debe generarse desde el borde derecho de la foto original hasta el nuevo límite extremo derecho del lienzo 16:9:
* **Continuación inmediata (zona de transición de los primeros milímetros):** Cómo empatan las líneas, texturas y sombras que cruzan el borde derecho.
* **Desarrollo del plano (Fondo, Plano Medio y Primer Plano):** Qué elementos coherentes con el entorno se extienden hacia la derecha manteniendo el equilibrio visual del encuadre 16:9.
* **Comportamiento de la luz:** Proyección de sombras y reflejos específicos en este sector derecho.

### 5. PROMPTS LISTOS PARA MOTOR DE GENERACIÓN (INGLÉS TÉCNICO)
Genera 3 prompts en inglés optimizados para modelos de difusión (FLUX.1 / Stable Diffusion XL Inpainting):
* **Full 16:9 Scene Prompt:** Descripción completa de la escena panorámica 16:9 unificada.
* **Left Wing Outpaint Prompt:** Prompt específico enfocado únicamente en rellenar el bloque izquierdo.
* **Right Wing Outpaint Prompt:** Prompt específico enfocado únicamente en rellenar el bloque derecho.

Devuelve la respuesta exclusivamente en formato JSON válido con las claves: optical_audit, left_panel_description, right_panel_description y flux_outpaint_prompt."#;

/// Everything that can happen to the AI expand dialog.
#[derive(Clone, Debug)]
pub enum AiExpandMsg {
    /// Opens the dialog for a specific clip.
    Open { clip_id: String },
    /// Dismisses the dialog.
    Dismiss,
    /// User updated the optional prompt hints.
    PromptChanged(String),
    /// Fill mode changed (0 = Desenfoque Bokeh, 1 = Reflejo Espejado, 2 = IA Generativa).
    FillModeChanged(i32),
    /// Intensity / Feathering level changed (0 = Ligero, 1 = Equilibrado, 2 = Profundo).
    FeatherChanged(i32),
    /// Starts the expansion process / applies the result.
    Generate,
    /// Background AI generation completed.
    GenerationDone {
        #[allow(dead_code)]
        clip_id: String,
        #[allow(dead_code)]
        original_media_id: String,
        result: Result<PathBuf, String>,
        cached_bg: Option<Vec<u8>>,
    },
    /// Reverts the clip back to the original vertical photo.
    RestoreOriginal,
}

/// State of the AI Expand dialog.
#[derive(Clone, Debug, Default)]
pub struct AiExpandPane {
    pub open: bool,
    pub clip_id: String,
    pub clip_name: String,
    pub photo: Option<slint::Image>,
    pub has_photo: bool,
    pub photo_width: f32,
    pub photo_height: f32,
    pub preview_photo: Option<slint::Image>,
    pub has_preview: bool,
    pub busy: bool,
    pub status_text: String,
    pub prompt: String,
    pub has_applied: bool,
    pub original_media_id: String,
    pub original_media_path: String,
    pub fill_mode: i32,
    pub feather_level: i32,
    pub cached_ai_bg: Option<Vec<u8>>,
}

impl AiExpandPane {
    /// Formats data for Slint UI binding.
    pub fn data(&self) -> AiExpandData {
        AiExpandData {
            open: self.open,
            clip_id: self.clip_id.clone().into(),
            clip_name: self.clip_name.clone().into(),
            photo: self.photo.clone().unwrap_or_default(),
            has_photo: self.has_photo,
            photo_width: self.photo_width,
            photo_height: self.photo_height,
            preview_photo: self.preview_photo.clone().unwrap_or_default(),
            has_preview: self.has_preview,
            busy: self.busy,
            status_text: self.status_text.clone().into(),
            prompt: self.prompt.clone().into(),
            has_applied: self.has_applied,
            fill_mode: self.fill_mode,
            feather_level: self.feather_level,
        }
    }

    /// Prepares and opens the dialog for a chosen clip.
    pub fn open_for_clip(&mut self, studio: &mut Studio, clip_id: &str) {
        let Some(clip) = studio.timeline().clips.iter().find(|c| c.id == clip_id).cloned() else {
            studio.notify("No se encontró el clip seleccionado.", true);
            return;
        };

        self.clip_id = clip_id.to_string();
        self.clip_name = clip.name.clone();
        self.busy = false;
        self.status_text.clear();
        self.preview_photo = None;
        self.has_preview = false;
        self.fill_mode = 2; // Default to IA Generativa Outpainting
        self.feather_level = 1; // Equilibrado
        self.prompt.clear();
        self.cached_ai_bg = None;

        // Check if this clip already has an applied ai_expand filter
        let existing_filter = clip.video_effects.iter().find(|e| e.id == "ai_expand").cloned();
        self.has_applied = existing_filter.is_some();
        if let Some(ref filter) = existing_filter {
            if let Some(&mode_val) = filter.params.get("mode") {
                self.fill_mode = mode_val as i32;
            }
            if let Some(&f_val) = filter.params.get("feather") {
                self.feather_level = f_val as i32;
            }
        }

        // Determine the original media ID and current media
        let original_media_id = if let Some(ref filter) = existing_filter {
            filter.source_media_id.clone().unwrap_or_else(|| clip.media_id.clone())
        } else {
            clip.media_id.clone()
        };
        self.original_media_id = original_media_id.clone();

        // Find the original media path
        let original_path = studio
            .session
            .as_ref()
            .and_then(|s| s.project().media_by_id(&original_media_id))
            .map(|m| m.path.clone());

        if let Some(path_str) = original_path {
            self.original_media_path = path_str.clone();
            let path = Path::new(&path_str);
            if path.exists() {
                if let Ok(img) = image::open(path) {
                    self.photo_width = img.width() as f32;
                    self.photo_height = img.height() as f32;
                    self.has_photo = true;
                    if let Ok(slint_img) = slint::Image::load_from_path(path) {
                        self.photo = Some(slint_img);
                    }
                }
            }
        }

        // If it was already applied and we have the expanded clip active, load it
        if self.has_applied && clip.media_id != self.original_media_id {
            if let Some(curr_path) = studio
                .session
                .as_ref()
                .and_then(|s| s.project().media_by_id(&clip.media_id))
                .map(|m| m.path.clone())
            {
                if let Ok(slint_img) = slint::Image::load_from_path(Path::new(&curr_path)) {
                    self.preview_photo = Some(slint_img);
                    self.has_preview = true;
                }
            }
        }

        // Generate instant preview immediately so user sees the 16:9 look right away!
        if !self.has_preview && self.has_photo {
            self.update_instant_preview();
        }

        self.open = true;
    }

    /// Updates instant live preview when mode or intensity changes.
    pub fn update_instant_preview(&mut self) {
        if self.original_media_path.is_empty() {
            return;
        }
        let path = Path::new(&self.original_media_path);
        if !path.exists() {
            return;
        }

        match self.fill_mode {
            0 => {
                // Mode 0: Desenfoque Cinemático Bokeh
                if let Ok(composite) = create_bokeh_composite(path, self.feather_level) {
                    let w = composite.width();
                    let h = composite.height();
                    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        composite.as_raw(),
                        w,
                        h,
                    );
                    self.preview_photo = Some(slint::Image::from_rgba8(buffer));
                    self.has_preview = true;
                }
            }
            1 => {
                // Mode 1: Reflejo Espejado
                if let Ok(composite) = create_mirrored_composite(path, self.feather_level) {
                    let w = composite.width();
                    let h = composite.height();
                    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        composite.as_raw(),
                        w,
                        h,
                    );
                    self.preview_photo = Some(slint::Image::from_rgba8(buffer));
                    self.has_preview = true;
                }
            }
            2 => {
                // Mode 2: IA Generativa (re-blend with cached background if available)
                if let Some(ref bg_bytes) = self.cached_ai_bg {
                    if let Ok(composite_bytes) = blend_composite(path, bg_bytes, self.feather_level) {
                        if let Ok(dyn_img) = image::load_from_memory(&composite_bytes) {
                            let rgba = dyn_img.to_rgba8();
                            let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                rgba.as_raw(),
                                rgba.width(),
                                rgba.height(),
                            );
                            self.preview_photo = Some(slint::Image::from_rgba8(buffer));
                            self.has_preview = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles events dispatched to the AI expand controller.
    pub fn update(&mut self, msg: AiExpandMsg, studio: &mut Studio) {
        match msg {
            AiExpandMsg::Open { clip_id } => {
                self.open_for_clip(studio, &clip_id);
            }
            AiExpandMsg::Dismiss => {
                self.open = false;
                self.busy = false;
            }
            AiExpandMsg::PromptChanged(p) => {
                self.prompt = p;
            }
            AiExpandMsg::FillModeChanged(m) => {
                self.fill_mode = m;
                self.update_instant_preview();
            }
            AiExpandMsg::FeatherChanged(f) => {
                self.feather_level = f;
                self.update_instant_preview();
            }
            AiExpandMsg::Generate => {
                if self.original_media_path.is_empty() {
                    studio.notify("No se encontró la ruta de la imagen original.", true);
                    return;
                }

                // Modes 0 (Bokeh) & 1 (Mirrored) process instantly without external API!
                if self.fill_mode == 0 || self.fill_mode == 1 {
                    let path = Path::new(&self.original_media_path);
                    let res = if self.fill_mode == 0 {
                        create_bokeh_composite(path, self.feather_level)
                    } else {
                        create_mirrored_composite(path, self.feather_level)
                    };

                    match res {
                        Ok(img) => {
                            let cache_dir = get_cache_dir(studio);
                            let _ = std::fs::create_dir_all(&cache_dir);
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            let target_path = cache_dir.join(format!(
                                "ai_expand_{}_{timestamp}.jpg",
                                self.clip_id
                            ));
                            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
                            if let Err(e) = rgb.save(&target_path) {
                                studio.notify(&format!("Error al guardar imagen: {e}"), true);
                                return;
                            }
                            self.apply_target_image(studio, &target_path);
                            return;
                        }
                        Err(err) => {
                            studio.notify(&err, true);
                            return;
                        }
                    }
                }

                // Mode 2: IA Generativa Outpainting (Direct Gemini outpainting or Vision + Synthesis)
                self.busy = true;
                self.status_text = "Generando outpainting 16:9 con IA y armonizando iluminación...".to_string();

                let clip_id = self.clip_id.clone();
                let original_media_id = self.original_media_id.clone();
                let original_path_str = self.original_media_path.clone();
                let user_prompt = self.prompt.clone();
                let clip_name = self.clip_name.clone();
                let feather_level = self.feather_level;
                let cache_dir = get_cache_dir(studio);

                let token_opt = studio.assistant.auth.get_valid_token(&studio.host.dirs).ok();
                let quota_proj = studio.assistant.auth.quota_project();

                let clip_id_worker = clip_id.clone();
                crate::host::spawn_in_project(
                    move || {
                        let path = Path::new(&original_path_str);
                        let img_bytes = std::fs::read(path)
                            .map_err(|e| format!("Error al leer la foto original: {e}"))?;

                        // 1. Try direct multimodal image outpainting first (for billed GCP / Gemini image accounts)
                        let direct_img = if let Some(ref token) = token_opt {
                            let client = GeminiClient::new(token.clone()).with_quota_project(quota_proj.clone());
                            try_direct_gemini_outpainting(&client, &img_bytes, &user_prompt).ok()
                        } else {
                            None
                        };

                        let (composite_bytes, bg_bytes) = if let Some(direct_bytes) = direct_img {
                            log::info!("ai_expand: direct multimodal image outpainting succeeded!");
                            let blended = blend_composite(path, &direct_bytes, feather_level)
                                .unwrap_or_else(|_| direct_bytes.clone());
                            (blended, direct_bytes)
                        } else {
                            // 2. Multimodal vision-guided synthesis pipeline
                            let vision_prompt = if let Some(ref token) = token_opt {
                                let client = GeminiClient::new(token.clone()).with_quota_project(quota_proj);
                                match query_gemini_vision(&client, &img_bytes, &user_prompt) {
                                    Ok(p) => {
                                        log::info!("ai_expand: Gemini Vision prompt: {p}");
                                        p
                                    }
                                    Err(err) => {
                                        log::warn!("ai_expand: Gemini Vision fallback ({err})");
                                        build_fallback_prompt(&clip_name, &user_prompt)
                                    }
                                }
                            } else {
                                build_fallback_prompt(&clip_name, &user_prompt)
                            };

                            log::info!("ai_expand: Gemini analysis completed:\n{vision_prompt}");

                            let diffusion_prompt = extract_diffusion_prompt(&vision_prompt);
                            log::info!("ai_expand: generating lateral 16:9 background with prompt: {diffusion_prompt}");

                            let bg = fetch_ai_image(&diffusion_prompt, 1280, 720)
                                .map_err(|e| format!("Error al generar laterales con IA: {e}"))?;

                            let composite = blend_composite(path, &bg, feather_level)?;
                            (composite, bg)
                        };

                        let _ = std::fs::create_dir_all(&cache_dir);
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let target_path = cache_dir.join(format!("ai_expand_{}_{timestamp}.jpg", clip_id_worker));

                        std::fs::write(&target_path, &composite_bytes)
                            .map_err(|e| format!("Error al guardar imagen expandida: {e}"))?;

                        Ok((target_path, bg_bytes))
                    },
                    move |studio: &mut Studio, _, _, result| {
                        let (target_res, cached_bg) = match result {
                            Ok((path, bg)) => (Ok(path), Some(bg)),
                            Err(err) => (Err(err), None),
                        };
                        studio.handle(crate::panes::Msg::AiExpand(AiExpandMsg::GenerationDone {
                            clip_id,
                            original_media_id,
                            result: target_res,
                            cached_bg,
                        }));
                    },
                );
            }
            AiExpandMsg::GenerationDone {
                clip_id: _,
                original_media_id: _,
                result,
                cached_bg,
            } => {
                self.busy = false;
                self.status_text.clear();
                self.cached_ai_bg = cached_bg;

                match result {
                    Ok(target_path) => {
                        self.apply_target_image(studio, &target_path);
                    }
                    Err(err) => {
                        studio.notify(&err, true);
                    }
                }
            }
            AiExpandMsg::RestoreOriginal => {
                self.restore_original(studio);
            }
        }
    }

    /// Imports the target expanded image and updates the active clip in the project.
    fn apply_target_image(&mut self, studio: &mut Studio, target_path: &Path) {
        let path_str = target_path.to_string_lossy().into_owned();

        let Ok(summary) = concat_host::media::probe(&path_str) else {
            studio.notify("No se pudo analizar la imagen generada", true);
            return;
        };

        let Some(new_media_id) = studio.apply(Command::AddMedia {
            item: summary.to_new_media(),
        }) else {
            studio.notify("Error agregando imagen al proyecto", true);
            return;
        };

        let Some(session) = studio.session.as_mut() else { return };

        let Some(clip) = session.project().active().clip(&self.clip_id).cloned() else { return };
        let mut video_effects = clip.video_effects.clone();

        let mut filter = AppliedFilter::new("ai_expand");
        filter.source_media_id = Some(self.original_media_id.clone());
        filter.params.insert("expanded".to_string(), 1.0);
        filter.params.insert("mode".to_string(), self.fill_mode as f64);
        filter.params.insert("feather".to_string(), self.feather_level as f64);

        if let Some(pos) = video_effects.iter().position(|e| e.id == "ai_expand") {
            video_effects[pos] = filter;
        } else {
            video_effects.push(filter);
        }

        let patch = ClipPatch {
            media_id: Some(new_media_id),
            video_effects: Some(video_effects),
            ..Default::default()
        };

        let _ = studio.apply(Command::UpdateClip {
            clip_id: self.clip_id.clone(),
            patch,
        });

        self.has_applied = true;
        if let Ok(slint_img) = slint::Image::load_from_path(target_path) {
            self.preview_photo = Some(slint_img);
            self.has_preview = true;
        }

        self.open = false;
        let mode_desc = match self.fill_mode {
            0 => "Desenfoque Cinemático Bokeh",
            1 => "Reflejo Espejado Suave",
            _ => "IA Generativa Outpainting",
        };
        studio.notify(&format!("✨ Foto vertical adaptada a 16:9 ({mode_desc}) exitosamente"), false);
        studio.request_preview();
    }

    /// Reverts the clip back to the original vertical photo.
    pub fn restore_original(&mut self, studio: &mut Studio) {
        let clip_id = if !self.clip_id.is_empty() {
            self.clip_id.clone()
        } else if let Some(id) = studio.sole_selection() {
            id
        } else {
            studio.notify("No se encontró el clip seleccionado.", true);
            return;
        };

        let Some(clip) = studio.timeline().clips.iter().find(|c| c.id == clip_id).cloned() else {
            return;
        };

        let original_media_id = if !self.original_media_id.is_empty() {
            self.original_media_id.clone()
        } else if let Some(filter) = clip.video_effects.iter().find(|e| e.id == "ai_expand") {
            filter.source_media_id.clone().unwrap_or_default()
        } else {
            String::new()
        };

        if original_media_id.is_empty() {
            studio.notify("No se encontró la foto original a restaurar.", true);
            return;
        }

        // Remove ai_expand from video effects
        let mut video_effects = clip.video_effects.clone();
        video_effects.retain(|e| e.id != "ai_expand");

        let patch = ClipPatch {
            media_id: Some(original_media_id),
            video_effects: Some(video_effects),
            ..Default::default()
        };

        let _ = studio.apply(Command::UpdateClip {
            clip_id: clip_id.clone(),
            patch,
        });

        self.has_applied = false;
        self.preview_photo = None;
        self.has_preview = false;
        self.cached_ai_bg = None;
        self.open = false;

        studio.notify("Efecto de Expansión eliminado: foto restaurada a la original", false);
        studio.request_preview();
    }
}

/// Helper to get the AI cache directory.
fn get_cache_dir(studio: &Studio) -> PathBuf {
    if let Some(session) = studio.session.as_ref() {
        let p = session.path();
        if !p.is_empty() && Path::new(p).exists() {
            Path::new(p).join("ai_generated")
        } else {
            studio.host.dirs.data.join("ai_generated")
        }
    } else {
        studio.host.dirs.data.join("ai_generated")
    }
}

/// Erases bottom-right pollinations watermark by copying horizontally from the clean left edge at the same scanline height.
fn erase_watermark(bg: &mut image::RgbaImage) {
    let w = bg.width();
    let h = bg.height();
    if w < 1000 || h < 600 {
        return;
    }

    let start_x = w.saturating_sub(185);
    let start_y = h.saturating_sub(42);

    for y in start_y..h {
        let reference_p = *bg.get_pixel(start_x.saturating_sub(1), y);
        for x in start_x..w {
            bg.put_pixel(x, y, reference_p);
        }
    }
}

/// Computes mean and standard deviation for RGB channels within specified regions.
fn compute_stats_for_region(
    img: &image::RgbaImage,
    x_range1: std::ops::Range<u32>,
    x_range2: std::ops::Range<u32>,
    y_range: std::ops::Range<u32>,
) -> ([f32; 3], [f32; 3]) {
    let mut sum = [0.0f32; 3];
    let mut count = 0.0f32;

    for y in y_range.clone() {
        if y >= img.height() {
            continue;
        }
        for x in x_range1.clone() {
            if x >= img.width() {
                continue;
            }
            let p = img.get_pixel(x, y);
            sum[0] += p[0] as f32;
            sum[1] += p[1] as f32;
            sum[2] += p[2] as f32;
            count += 1.0;
        }
        for x in x_range2.clone() {
            if x >= img.width() {
                continue;
            }
            let p = img.get_pixel(x, y);
            sum[0] += p[0] as f32;
            sum[1] += p[1] as f32;
            sum[2] += p[2] as f32;
            count += 1.0;
        }
    }

    if count < 1.0 {
        return ([128.0, 128.0, 128.0], [50.0, 50.0, 50.0]);
    }

    let mean = [sum[0] / count, sum[1] / count, sum[2] / count];

    let mut var_sum = [0.0f32; 3];
    for y in y_range {
        if y >= img.height() {
            continue;
        }
        for x in x_range1.clone() {
            if x >= img.width() {
                continue;
            }
            let p = img.get_pixel(x, y);
            for c in 0..3 {
                let diff = p[c] as f32 - mean[c];
                var_sum[c] += diff * diff;
            }
        }
        for x in x_range2.clone() {
            if x >= img.width() {
                continue;
            }
            let p = img.get_pixel(x, y);
            for c in 0..3 {
                let diff = p[c] as f32 - mean[c];
                var_sum[c] += diff * diff;
            }
        }
    }

    let std = [
        (var_sum[0] / count).sqrt().max(1.0),
        (var_sum[1] / count).sqrt().max(1.0),
        (var_sum[2] / count).sqrt().max(1.0),
    ];

    (mean, std)
}

/// Harmonizes AI background with the photo using continuous vertical interpolation.
/// NO hard step at y=360! This prevents any horizontal line or bar across the image.
fn harmonize_background_continuous(orig: &image::RgbaImage, bg: &mut image::RgbaImage) {
    let orig_w = orig.width();
    let orig_h = orig.height();
    let bg_w = bg.width();
    let bg_h = bg.height();

    if orig_w == 0 || orig_h == 0 || bg_w == 0 || bg_h == 0 {
        return;
    }

    let half_orig_h = orig_h / 2;
    let half_bg_h = bg_h / 2;

    let strip_w = (orig_w / 4).max(2).min(orig_w / 2);

    let (orig_top_mean, orig_top_std) = compute_stats_for_region(
        orig,
        0..strip_w,
        orig_w.saturating_sub(strip_w)..orig_w,
        0..half_orig_h,
    );
    let (orig_bot_mean, orig_bot_std) = compute_stats_for_region(
        orig,
        0..strip_w,
        orig_w.saturating_sub(strip_w)..orig_w,
        half_orig_h..orig_h,
    );

    let scale_factor = bg_h as f32 / orig_h as f32;
    let scaled_orig_w = ((orig_w as f32 * scale_factor).round() as u32).min(bg_w);
    let offset_x = (bg_w.saturating_sub(scaled_orig_w)) / 2;
    let right_start = offset_x + scaled_orig_w;

    let (bg_top_mean, bg_top_std) = compute_stats_for_region(
        bg,
        0..offset_x,
        right_start..bg_w,
        0..half_bg_h,
    );
    let (bg_bot_mean, bg_bot_std) = compute_stats_for_region(
        bg,
        0..offset_x,
        right_start..bg_w,
        half_bg_h..bg_h,
    );

    for y in 0..bg_h {
        // Continuous smooth vertical gradient: prevents any abrupt horizontal line
        let t = (y as f32 / bg_h as f32).clamp(0.0, 1.0);
        let orig_mean = [
            orig_top_mean[0] * (1.0 - t) + orig_bot_mean[0] * t,
            orig_top_mean[1] * (1.0 - t) + orig_bot_mean[1] * t,
            orig_top_mean[2] * (1.0 - t) + orig_bot_mean[2] * t,
        ];
        let orig_std = [
            orig_top_std[0] * (1.0 - t) + orig_bot_std[0] * t,
            orig_top_std[1] * (1.0 - t) + orig_bot_std[1] * t,
            orig_top_std[2] * (1.0 - t) + orig_bot_std[2] * t,
        ];
        let bg_mean = [
            bg_top_mean[0] * (1.0 - t) + bg_bot_mean[0] * t,
            bg_top_mean[1] * (1.0 - t) + bg_bot_mean[1] * t,
            bg_top_mean[2] * (1.0 - t) + bg_bot_mean[2] * t,
        ];
        let bg_std = [
            bg_top_std[0] * (1.0 - t) + bg_bot_std[0] * t,
            bg_top_std[1] * (1.0 - t) + bg_bot_std[1] * t,
            bg_top_std[2] * (1.0 - t) + bg_bot_std[2] * t,
        ];

        for x in 0..bg_w {
            let p = bg.get_pixel_mut(x, y);
            for c in 0..3 {
                let val = p[c] as f32;
                let std_ratio = (orig_std[c] / bg_std[c]).clamp(0.6, 1.8);
                let transferred = orig_mean[c] + (val - bg_mean[c]) * std_ratio;
                // Subtle 35% color temperature alignment
                let blended = val * 0.65 + transferred * 0.35;
                p[c] = blended.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Composites the central vertical photo:
/// - 100% SHARP and 100% OPAQUE (NO translucent ghost arms or faces!).
/// - Elegant drop shadow behind the photo borders to separate foreground from ambient fill.
/// Composites the central vertical photo:
/// - 100% SHARP and 100% OPAQUE in the center (NO translucent ghost arms or faces!).
/// - Optional soft drop shadow when framed, or seamless smoothstep alpha blend for outpainting.
fn composite_center_photo(
    orig_dyn: &image::RgbaImage,
    bg: &mut image::RgbaImage,
    with_shadow: bool,
    feather_width: u32,
) {
    let target_w = bg.width();
    let target_h = bg.height();

    let orig_w = orig_dyn.width() as f32;
    let orig_h = orig_dyn.height() as f32;
    let scale_factor = target_h as f32 / orig_h;
    let scaled_orig_w = ((orig_w * scale_factor).round() as u32).clamp(1, target_w);
    let scaled_orig_h = target_h;

    let scaled_orig = image::imageops::resize(
        orig_dyn,
        scaled_orig_w,
        scaled_orig_h,
        image::imageops::FilterType::Lanczos3,
    );

    let offset_x = (target_w.saturating_sub(scaled_orig_w)) / 2;
    let right_edge = offset_x + scaled_orig_w;

    // 1. Soft drop shadow on background to frame the central vertical photo (only when explicitly requested)
    if with_shadow {
        let shadow_radius = 22u32;
        // Left shadow
        for d in 0..shadow_radius {
            let sx = offset_x.saturating_sub(1 + d);
            if sx < target_w {
                let factor = 1.0 - (1.0 - (d as f32 / shadow_radius as f32)).powi(2) * 0.42;
                for y in 0..target_h {
                    let p = bg.get_pixel_mut(sx, y);
                    p[0] = ((p[0] as f32) * factor) as u8;
                    p[1] = ((p[1] as f32) * factor) as u8;
                    p[2] = ((p[2] as f32) * factor) as u8;
                }
            }
        }
        // Right shadow
        for d in 0..shadow_radius {
            let sx = right_edge + d;
            if sx < target_w {
                let factor = 1.0 - (1.0 - (d as f32 / shadow_radius as f32)).powi(2) * 0.42;
                for y in 0..target_h {
                    let p = bg.get_pixel_mut(sx, y);
                    p[0] = ((p[0] as f32) * factor) as u8;
                    p[1] = ((p[1] as f32) * factor) as u8;
                    p[2] = ((p[2] as f32) * factor) as u8;
                }
            }
        }
    }

    // 2. Paste center photo with cubic smoothstep alpha edge blending
    let feather_px = feather_width.max(1);

    for y in 0..target_h {
        for x in 0..scaled_orig_w {
            let dest_x = offset_x + x;
            if dest_x >= target_w {
                continue;
            }

            let orig_p = scaled_orig.get_pixel(x, y);

            if feather_px <= 1 || (x >= feather_px && x < scaled_orig_w - feather_px) {
                // 100% solid, sharp original photo
                bg.put_pixel(dest_x, y, *orig_p);
            } else {
                let t = if x < feather_px {
                    x as f32 / feather_px as f32
                } else {
                    (scaled_orig_w - 1 - x) as f32 / feather_px as f32
                };
                let alpha = (t * t * (3.0 - 2.0 * t)).clamp(0.0, 1.0);
                let bg_p = bg.get_pixel(dest_x, y);
                let r = (orig_p[0] as f32 * alpha + bg_p[0] as f32 * (1.0 - alpha)).round() as u8;
                let g = (orig_p[1] as f32 * alpha + bg_p[1] as f32 * (1.0 - alpha)).round() as u8;
                let b = (orig_p[2] as f32 * alpha + bg_p[2] as f32 * (1.0 - alpha)).round() as u8;
                bg.put_pixel(dest_x, y, image::Rgba([r, g, b, 255]));
            }
        }
    }
}

/// Blends the original photo seamlessly into the AI background with continuous harmonization and invisible seam.
fn blend_composite(orig_path: &Path, bg_bytes: &[u8], feather_level: i32) -> Result<Vec<u8>, String> {
    let orig_dyn = image::open(orig_path)
        .map_err(|e| format!("Error decodificando foto original: {e}"))?
        .to_rgba8();

    let bg_dyn = image::load_from_memory(bg_bytes)
        .map_err(|e| format!("Error decodificando fondo generado: {e}"))?
        .to_rgba8();

    let target_w: u32 = 1280;
    let target_h: u32 = 720;

    let mut bg = if bg_dyn.width() == target_w && bg_dyn.height() == target_h {
        bg_dyn
    } else {
        image::imageops::resize(&bg_dyn, target_w, target_h, image::imageops::FilterType::Lanczos3)
    };

    // 0. Clean any bottom-right corner watermark logo
    erase_watermark(&mut bg);

    // 1. Continuous vertical harmonization across background
    harmonize_background_continuous(&orig_dyn, &mut bg);

    // 2. Feather width based on feather_level (8px, 18px, 32px)
    let feather_width = match feather_level {
        0 => 8u32,
        2 => 32u32,
        _ => 18u32,
    };

    // 3. Composite center photo with NO shadow (seamless outpainting) and smooth smoothstep transition
    composite_center_photo(&orig_dyn, &mut bg, false, feather_width);

    let mut output_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut output_bytes);
    let rgb = image::DynamicImage::ImageRgba8(bg).to_rgb8();
    rgb.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Error guardando imagen combinada en JPEG: {e}"))?;

    Ok(output_bytes)
}

/// Ultra-fast separable box blur for downscaled ambient backgrounds.
fn fast_box_blur(img: &mut image::RgbaImage, radius: usize) {
    let w = img.width() as usize;
    let h = img.height() as usize;
    if w == 0 || h == 0 || radius == 0 {
        return;
    }

    let mut temp = img.clone();
    for y in 0..h {
        for x in 0..w {
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius).min(w - 1);
            let count = (x_end - x_start + 1) as u32;

            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;

            for kx in x_start..=x_end {
                let p = img.get_pixel(kx as u32, y as u32);
                r_sum += p[0] as u32;
                g_sum += p[1] as u32;
                b_sum += p[2] as u32;
            }

            temp.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([
                    (r_sum / count) as u8,
                    (g_sum / count) as u8,
                    (b_sum / count) as u8,
                    255,
                ]),
            );
        }
    }

    for x in 0..w {
        for y in 0..h {
            let y_start = y.saturating_sub(radius);
            let y_end = (y + radius).min(h - 1);
            let count = (y_end - y_start + 1) as u32;

            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;

            for ky in y_start..=y_end {
                let p = temp.get_pixel(x as u32, ky as u32);
                r_sum += p[0] as u32;
                g_sum += p[1] as u32;
                b_sum += p[2] as u32;
            }

            img.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([
                    (r_sum / count) as u8,
                    (g_sum / count) as u8,
                    (b_sum / count) as u8,
                    255,
                ]),
            );
        }
    }
}

/// Creates a professional Cinematic Ambient Bokeh composite (Mode 0 - Recommended & Instant).
fn create_bokeh_composite(orig_path: &Path, intensity_level: i32) -> Result<image::RgbaImage, String> {
    let orig_dyn = image::open(orig_path)
        .map_err(|e| format!("Error abriendo imagen original: {e}"))?
        .to_rgba8();

    let target_w: u32 = 1280;
    let target_h: u32 = 720;

    let orig_w = orig_dyn.width();
    let orig_h = orig_dyn.height();
    if orig_w == 0 || orig_h == 0 {
        return Err("Dimensiones inválidas en imagen original".to_string());
    }

    // 1. Scale to cover 1280x720
    let scale_cover = (target_w as f32 / orig_w as f32).max(target_h as f32 / orig_h as f32);
    let cover_w = (orig_w as f32 * scale_cover).round() as u32;
    let cover_h = (orig_h as f32 * scale_cover).round() as u32;

    let cover_scaled = image::imageops::resize(&orig_dyn, cover_w, cover_h, image::imageops::FilterType::Triangle);

    let crop_x = (cover_w.saturating_sub(target_w)) / 2;
    let crop_y = (cover_h.saturating_sub(target_h)) / 2;
    let bg_cropped = image::imageops::crop_imm(&cover_scaled, crop_x, crop_y, target_w, target_h).to_image();

    // 2. Downscale to 160x90 for fast Gaussian bokeh approximation
    let mut small = image::imageops::resize(&bg_cropped, 160, 90, image::imageops::FilterType::Triangle);
    let (blur_radius, blur_passes) = match intensity_level {
        0 => (3, 1),
        2 => (6, 3),
        _ => (5, 2),
    };
    for _ in 0..blur_passes {
        fast_box_blur(&mut small, blur_radius);
    }

    let mut bg = image::imageops::resize(&small, target_w, target_h, image::imageops::FilterType::Triangle);

    // Subtle 12% ambient dimming so center vertical photo pops out with clean depth
    for p in bg.pixels_mut() {
        p[0] = ((p[0] as f32) * 0.88) as u8;
        p[1] = ((p[1] as f32) * 0.88) as u8;
        p[2] = ((p[2] as f32) * 0.88) as u8;
    }

    // 3. Composite crisp center photo with framing shadow
    composite_center_photo(&orig_dyn, &mut bg, true, 4);

    Ok(bg)
}

/// Creates a Mirrored composite (Mode 1) with soft blur on lateral extensions.
fn create_mirrored_composite(orig_path: &Path, intensity_level: i32) -> Result<image::RgbaImage, String> {
    let orig_dyn = image::open(orig_path)
        .map_err(|e| format!("Error abriendo imagen original: {e}"))?
        .to_rgba8();

    let target_w: u32 = 1280;
    let target_h: u32 = 720;

    let orig_w = orig_dyn.width() as f32;
    let orig_h = orig_dyn.height() as f32;
    let scale_factor = target_h as f32 / orig_h;
    let scaled_orig_w = ((orig_w * scale_factor).round() as u32).clamp(1, target_w);
    let scaled_orig_h = target_h;

    let scaled_orig = image::imageops::resize(
        &orig_dyn,
        scaled_orig_w,
        scaled_orig_h,
        image::imageops::FilterType::Lanczos3,
    );

    let offset_x = (target_w.saturating_sub(scaled_orig_w)) / 2;
    let right_start = offset_x + scaled_orig_w;
    let mut bg = image::RgbaImage::new(target_w, target_h);

    // Mirror left side
    for y in 0..target_h {
        for x in 0..offset_x {
            let dist = offset_x.saturating_sub(1 + x);
            let src_x = dist.min(scaled_orig_w - 1);
            let p = scaled_orig.get_pixel(src_x, y);
            bg.put_pixel(x, y, *p);
        }
    }

    // Mirror right side
    for y in 0..target_h {
        for x in right_start..target_w {
            let dist = x - right_start;
            let src_x = (scaled_orig_w - 1).saturating_sub(dist);
            let p = scaled_orig.get_pixel(src_x, y);
            bg.put_pixel(x, y, *p);
        }
    }

    // Soft blur the mirrored sides
    let mut small = image::imageops::resize(&bg, 160, 90, image::imageops::FilterType::Triangle);
    let (blur_radius, blur_passes) = match intensity_level {
        0 => (3, 1),
        2 => (6, 3),
        _ => (4, 2),
    };
    for _ in 0..blur_passes {
        fast_box_blur(&mut small, blur_radius);
    }
    let bg_blurred = image::imageops::resize(&small, target_w, target_h, image::imageops::FilterType::Triangle);

    for y in 0..target_h {
        for x in 0..target_w {
            if x < offset_x || x >= right_start {
                let p = bg_blurred.get_pixel(x, y);
                // 10% dimming for mirrored sides
                bg.put_pixel(x, y, image::Rgba([
                    ((p[0] as f32) * 0.90) as u8,
                    ((p[1] as f32) * 0.90) as u8,
                    ((p[2] as f32) * 0.90) as u8,
                    255,
                ]));
            }
        }
    }

    composite_center_photo(&orig_dyn, &mut bg, false, 8);

    Ok(bg)
}

/// Attempts direct multimodal image outpainting if Google Cloud project has image generation enabled.
fn try_direct_gemini_outpainting(
    client: &GeminiClient,
    img_bytes: &[u8],
    user_prompt: &str,
) -> Result<Vec<u8>, String> {
    let base64_data = BASE64.encode(img_bytes);
    let mut image_extra = serde_json::Map::new();
    image_extra.insert(
        "inlineData".to_string(),
        serde_json::json!({
            "mimeType": "image/jpeg",
            "data": base64_data,
        }),
    );

    let prompt = if user_prompt.trim().is_empty() {
        DEFAULT_OUTPAINTING_PROMPT
    } else {
        user_prompt.trim()
    };

    let parts = vec![
        Part {
            text: Some(prompt.to_string()),
            ..Default::default()
        },
        Part {
            extra: image_extra,
            ..Default::default()
        },
    ];

    let request = GenerateContentRequest {
        contents: vec![Content {
            role: "user".to_string(),
            parts,
        }],
        system_instruction: None,
        tools: None,
        generation_config: Some(serde_json::json!({
            "responseModalities": ["TEXT", "IMAGE"]
        })),
    };

    for model in ["gemini-nano-banana-2.1", "gemini-3.1-flash-image", "gemini-2.5-flash-image"] {
        if let Ok(resp) = client.generate_content(model, &request) {
            if let Some(candidates) = resp.candidates {
                for c in candidates {
                    if let Some(content) = c.content {
                        for p in content.parts {
                            if let Some(inline) = p.extra.get("inlineData").and_then(|v| v.as_object()) {
                                if let Some(data_str) = inline.get("data").and_then(|v| v.as_str()) {
                                    if let Ok(bytes) = BASE64.decode(data_str) {
                                        return Ok(bytes);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err("Generación directa de imagen no disponible".to_string())
}

/// Optimizes an image for Gemini Vision analysis by downscaling large camera photos to max 1280px.
fn prepare_vision_image(img_bytes: &[u8]) -> Vec<u8> {
    if let Ok(dyn_img) = image::load_from_memory(img_bytes) {
        let (w, h) = (dyn_img.width(), dyn_img.height());
        if w > 1280 || h > 1280 {
            let scale = 1280.0 / (w.max(h) as f32);
            let target_w = ((w as f32) * scale).round() as u32;
            let target_h = ((h as f32) * scale).round() as u32;
            let resized = dyn_img.resize(target_w, target_h, image::imageops::FilterType::Triangle);
            let mut out = Vec::new();
            let mut cursor = Cursor::new(&mut out);
            if resized.write_to(&mut cursor, image::ImageFormat::Jpeg).is_ok() {
                log::info!(
                    "prepare_vision_image: downscaled {w}x{h} ({} KB) to {target_w}x{target_h} ({} KB) for fast Gemini Vision upload",
                    img_bytes.len() / 1024,
                    out.len() / 1024
                );
                return out;
            }
        }
    }
    img_bytes.to_vec()
}

/// Sends image and user prompt to Gemini Vision (gemini-2.5-flash) to produce an outpainting description for lateral extension.
fn query_gemini_vision(
    client: &GeminiClient,
    raw_img_bytes: &[u8],
    user_hints: &str,
) -> Result<String, String> {
    let img_bytes = prepare_vision_image(raw_img_bytes);
    let base64_data = BASE64.encode(&img_bytes);

    let mut image_extra = serde_json::Map::new();
    image_extra.insert(
        "inlineData".to_string(),
        serde_json::json!({
            "mimeType": "image/jpeg",
            "data": base64_data,
        }),
    );

    let prompt_text = if user_hints.trim().is_empty() || user_hints.trim() == DEFAULT_OUTPAINTING_PROMPT {
        DEFAULT_OUTPAINTING_PROMPT.to_string()
    } else {
        format!(
            "{}\n\nDIRECTIVA ADICIONAL DEL USUARIO:\n\"{}\"",
            DEFAULT_OUTPAINTING_PROMPT,
            user_hints.trim()
        )
    };

    let parts = vec![
        Part {
            text: Some(prompt_text),
            ..Default::default()
        },
        Part {
            extra: image_extra,
            ..Default::default()
        },
    ];

    let request = GenerateContentRequest {
        contents: vec![Content {
            role: "user".to_string(),
            parts,
        }],
        system_instruction: Some(SystemInstruction::new(
            "You are a World-Class Director of Photography, VFX Outpainting Specialist, and Computer Vision Analyst. You analyze vertical source images and provide rigorous, millimeter-precise optical and spatial continuity specifications and diffusion prompts. Return your response exclusively as valid JSON with the requested keys.",
        )),
        tools: None,
        generation_config: Some(serde_json::json!({
            "responseMimeType": "application/json"
        })),
    };

    let response = client.generate_content("gemini-3-flash-preview", &request)?;

    let candidate = response
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content);

    let text = candidate
        .and_then(|c| c.parts.into_iter().next())
        .and_then(|p| p.text)
        .unwrap_or_default();

    if text.trim().is_empty() {
        return Err("Gemini devolvió una respuesta vacía".to_string());
    }

    Ok(text.trim().to_string())
}

/// Extracts the optimized English diffusion prompt from Gemini's technical VFX audit.
fn extract_diffusion_prompt(analysis: &str) -> String {
    let trimmed = analysis.trim();

    // 0. Primary parser: Direct JSON parsing
    let json_candidate = if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_candidate) {
        if let Some(prompt) = val.get("flux_outpaint_prompt").and_then(|v| v.as_str()) {
            if prompt.trim().len() > 15 {
                log::info!("ai_expand: successfully parsed JSON key 'flux_outpaint_prompt': {prompt}");
                return sanitize_diffusion_prompt(prompt);
            }
        }
    }

    let lower = analysis.to_lowercase();

    // 1. Look for Section 5
    if let Some(sec5_pos) = lower.find("### 5") {
        let sec5 = &analysis[sec5_pos..];

        // Preference order: empty background prompt, then right wing, then left wing
        for key in &["empty background", "background environment", "right wing outpaint", "left wing outpaint", "full 16:9", "scene prompt"] {
            if let Some(key_pos) = sec5.to_lowercase().find(key) {
                let block = &sec5[key_pos..];

                // If prompt is enclosed in backticks `...`
                if let Some(tick_start) = block.find('`') {
                    if let Some(tick_end) = block[tick_start + 1..].find('`') {
                        let candidate = block[tick_start + 1..tick_start + 1 + tick_end].trim();
                        if candidate.len() > 15 {
                            return sanitize_diffusion_prompt(candidate);
                        }
                    }
                }

                // If prompt is in a blockquote `> `
                if let Some(q_pos) = block.find("> ") {
                    let after_q = &block[q_pos + 2..];
                    let end_line = after_q.find('\n').unwrap_or(after_q.len());
                    let candidate = after_q[..end_line].trim().trim_matches(['`', '*', '"', '\'']);
                    if candidate.len() > 15 {
                        return sanitize_diffusion_prompt(candidate);
                    }
                }

                // If after label with colon (search for "prompt" followed by colon to avoid matching "16:9")
                if let Some(p_idx) = block.to_lowercase().find("prompt") {
                    if let Some(c_idx) = block[p_idx..].find(':') {
                        let after_c = &block[p_idx + c_idx + 1..];
                        let end_idx = after_c.find("\n*").or_else(|| after_c.find("\n#")).unwrap_or(after_c.len());
                        let candidate = after_c[..end_idx].trim().trim_matches(['`', '*', '>', '"', '\'', ' ', '\r', '\n']);
                        if candidate.len() > 15 {
                            return sanitize_diffusion_prompt(candidate);
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback: search for right wing or left wing prompts anywhere
    for key in &["right wing outpaint prompt", "left wing outpaint prompt"] {
        if let Some(pos) = lower.find(key) {
            let block = &analysis[pos..];
            if let Some(tick_start) = block.find('`') {
                if let Some(tick_end) = block[tick_start + 1..].find('`') {
                    let candidate = block[tick_start + 1..tick_start + 1 + tick_end].trim();
                    if candidate.len() > 15 {
                        return sanitize_diffusion_prompt(candidate);
                    }
                }
            }
        }
    }

    sanitize_diffusion_prompt("Seamless horizontal extension of the original background environment and surfaces, matching scale and lighting, photorealistic")
}

fn sanitize_diffusion_prompt(p: &str) -> String {
    let mut clean = p.trim()
        .trim_start_matches(['>', '*', '`', '"', '\'', ' ', '\r', '\n'])
        .trim_end_matches(['*', '`', '"', '\'', ' ', '\r', '\n'])
        .to_string();

    let lower = clean.to_lowercase();
    // If Gemini already outputted a clean background prompt with "no people" or "strictly empty", preserve it directly!
    if (lower.contains("no people") || lower.contains("strictly empty") || lower.contains("empty environment"))
        && !lower.contains("featuring a")
    {
        return clean;
    }

    // Only remove explicit foreground subject introductions
    for phrase in &[
        "featuring a young girl",
        "featuring a girl",
        "featuring a child",
        "featuring a person",
        "featuring a boy",
        "holding a boxer puppy",
        "holding an adorable boxer puppy",
        "holding a puppy",
        "holding a dog",
        "with a young girl",
        "with a girl",
        "with a child",
    ] {
        let mut cur_lower = clean.to_lowercase();
        while let Some(idx) = cur_lower.find(phrase) {
            clean.replace_range(idx..idx + phrase.len(), "");
            cur_lower = clean.to_lowercase();
        }
    }

    clean = clean
        .replace("  ", " ")
        .replace(" ,", ",")
        .replace(",,", ",");

    if clean.trim().len() < 20 {
        clean = "Seamless horizontal outpainting extension of the surrounding background textures, materials, and environment at matching 1:1 camera distance and perspective".to_string();
    }

    if !clean.to_lowercase().contains("no people") {
        clean.push_str(", strictly empty environment without people, no humans, no faces, no extra subjects, clean background");
    }
    clean
}

/// Builds a fallback prompt if Gemini Vision is unavailable.
fn build_fallback_prompt(clip_name: &str, user_hints: &str) -> String {
    let clean_name = clip_name
        .trim_end_matches(".jpg")
        .trim_end_matches(".png")
        .trim_end_matches(".jpeg")
        .replace(['_', '-'], " ");

    let prompt_base = if !user_hints.trim().is_empty() {
        user_hints.trim().to_string()
    } else {
        format!("seamless outpainted environment continuation extending {}", clean_name)
    };

    format!(
        "{}, matching scale, perspective, and lighting, clean background, strictly empty environment, no people, no watermarks",
        prompt_base
    )
}
