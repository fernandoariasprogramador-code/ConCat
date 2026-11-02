// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The interactive Face Zoom / Ken Burns dialog: animates scale and position
//! smoothly to focus on or reveal from a person's face.

use std::path::Path;
use std::rc::Rc;

use concat_project::model::{AppliedFilter, KeyEase, KeyProperty};
use concat_project::commands::ClipPatch;
use concat_project::Command;
use slint::{ModelRc, VecModel};

use crate::studio::Studio;
use crate::ui::{DetectedFaceData, FaceZoomData};

/// Everything that can happen to the face zoom dialog.
#[derive(Clone, Debug)]
pub enum FaceZoomMsg {
    /// Opens the dialog for the given clip.
    Open { clip_id: String },
    /// Dismisses the dialog without saving changes.
    Dismiss,
    /// Sets the target position on the photo (0..1 normalized).
    TargetChanged { x: f32, y: f32 },
    /// Changes the zoom factor (1.2 to 3.5).
    ZoomChanged(f32),
    /// Changes zoom direction (0: In, 1: Out).
    DirectionChanged(i32),
    /// Changes easing curve (0: Smooth/Cinematic, 1: Linear, 2: In-Out).
    EasingChanged(i32),
    /// Selects a specific detected face.
    FaceSelected(i32),
    /// Background face analysis finished.
    AnalysisFinished {
        clip_id: String,
        faces: Vec<concat_vision::DetectedFace>,
        dimensions: Option<(f32, f32)>,
    },
    /// Applies the zoom effect by setting keyframes on the clip.
    Apply,
}

/// The face zoom dialog's state.
#[derive(Clone, Debug)]
pub struct FaceZoomPane {
    pub open: bool,
    pub clip_id: String,
    pub clip_name: String,
    pub photo: Option<slint::Image>,
    pub has_photo: bool,
    pub photo_width: f32,
    pub photo_height: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub zoom: f32,
    pub direction: i32,
    pub easing: i32,
    pub detected_faces: Vec<concat_vision::DetectedFace>,
    pub selected_face: i32,
    pub is_cached: bool,
    pub analyzing: bool,
}

impl Default for FaceZoomPane {
    fn default() -> Self {
        Self {
            open: false,
            clip_id: String::new(),
            clip_name: String::new(),
            photo: None,
            has_photo: false,
            photo_width: 0.0,
            photo_height: 0.0,
            target_x: 0.5,
            target_y: 0.38, // slightly above center is standard face location
            zoom: 1.8,
            direction: 0, // Zoom In by default
            easing: 0,    // Smooth by default
            detected_faces: Vec::new(),
            selected_face: -1,
            is_cached: false,
            analyzing: false,
        }
    }
}

impl FaceZoomPane {
    /// Applies one message to update the pane's state.
    pub fn update(&mut self, msg: FaceZoomMsg, studio: &mut Studio) {
        match msg {
            FaceZoomMsg::Open { clip_id } => {
                self.open_for_clip(&clip_id, studio);
            }
            FaceZoomMsg::Dismiss => {
                self.open = false;
                self.analyzing = false;
            }
            FaceZoomMsg::TargetChanged { x, y } => {
                self.target_x = x.clamp(0.05, 0.95);
                self.target_y = y.clamp(0.05, 0.95);
                // If user clicks or drags onto a detected face box, select it, otherwise deselect
                self.selected_face = self
                    .detected_faces
                    .iter()
                    .find(|f| {
                        self.target_x >= f.x
                            && self.target_x <= f.x + f.w
                            && self.target_y >= f.y
                            && self.target_y <= f.y + f.h
                    })
                    .map(|f| f.id as i32)
                    .unwrap_or(-1);
            }
            FaceZoomMsg::ZoomChanged(zoom) => {
                self.zoom = zoom.clamp(1.1, 4.0);
            }
            FaceZoomMsg::DirectionChanged(dir) => {
                self.direction = dir;
            }
            FaceZoomMsg::EasingChanged(easing) => {
                self.easing = easing;
            }
            FaceZoomMsg::FaceSelected(id) => {
                self.selected_face = id;
                if let Some(face) = self.detected_faces.iter().find(|f| f.id as i32 == id) {
                    self.target_x = (face.x + face.w / 2.0).clamp(0.05, 0.95);
                    self.target_y = (face.y + face.h / 2.0).clamp(0.05, 0.95);
                }
            }
            FaceZoomMsg::AnalysisFinished {
                clip_id,
                faces,
                dimensions,
            } => {
                if self.clip_id == clip_id && self.open {
                    self.analyzing = false;
                    if let Some((w, h)) = dimensions {
                        if self.photo_width == 0.0 || self.photo_height == 0.0 {
                            self.photo_width = w;
                            self.photo_height = h;
                        }
                    }
                    self.detected_faces = faces;
                    if let Some(first_face) = self.detected_faces.first() {
                        self.selected_face = first_face.id as i32;
                        self.target_x = (first_face.x + first_face.w / 2.0).clamp(0.05, 0.95);
                        self.target_y = (first_face.y + first_face.h / 2.0).clamp(0.05, 0.95);
                    } else {
                        self.selected_face = -1;
                    }
                }
            }
            FaceZoomMsg::Apply => {
                self.apply_to_clip(studio);
            }
        }
    }

    fn open_for_clip(&mut self, clip_id: &str, studio: &mut Studio) {
        let target_clip_id = if clip_id.is_empty() {
            studio.sole_selection()
                .or_else(|| {
                    studio.timeline().clips.iter().find(|c| {
                        c.kind == concat_project::model::ClipKind::Image
                            && (c.start..=(c.start + c.duration)).contains(&(studio.playhead as f64))
                    }).map(|c| c.id.clone())
                })
                .or_else(|| {
                    studio.timeline().clips.iter().find(|c| c.kind == concat_project::model::ClipKind::Image).map(|c| c.id.clone())
                })
        } else {
            Some(clip_id.to_string())
        };

        let Some(clip_id_resolved) = target_clip_id else {
            studio.notify("Selecciona una foto en la línea de tiempo o arrastra el efecto sobre ella", true);
            return;
        };

        let Some(clip) = studio.project().active().clips.iter().find(|c| c.id == clip_id_resolved).cloned() else {
            return;
        };

        self.clip_id = clip_id_resolved;
        self.clip_name = clip.name.clone();
        self.open = true;
        self.detected_faces.clear();
        self.selected_face = -1;
        self.is_cached = false;
        self.photo_width = 0.0;
        self.photo_height = 0.0;
        self.analyzing = false;
        self.target_x = 0.5;
        self.target_y = 0.38;

        let mut had_saved_target = false;
        if let Some(existing) = clip.video_effects.iter().find(|e| e.id == "face_zoom") {
            if let Some(&dir) = existing.params.get("direction") {
                self.direction = dir as i32;
            }
            if let Some(&z) = existing.params.get("zoom") {
                self.zoom = z as f32;
            }
            if let Some(&tx) = existing.params.get("target_x") {
                self.target_x = tx as f32;
                had_saved_target = true;
            }
            if let Some(&ty) = existing.params.get("target_y") {
                self.target_y = ty as f32;
                had_saved_target = true;
            }
            if let Some(&e) = existing.params.get("easing") {
                self.easing = e as i32;
            }
            if let Some(&fid) = existing.params.get("face_id") {
                self.selected_face = fid as i32;
            }
        }

        // Try loading the media photo for preview and face detection
        if let Some(media) = studio.project().media_by_id(&clip.media_id) {
            if let Some(w) = media.width {
                self.photo_width = w as f32;
            }
            if let Some(h) = media.height {
                self.photo_height = h as f32;
            }

            let path = Path::new(&media.path);
            if path.is_file() {
                if let Ok(img) = slint::Image::load_from_path(path) {
                    self.photo = Some(img);
                    self.has_photo = true;
                }

                if self.photo_width == 0.0 || self.photo_height == 0.0 {
                    if let Ok((w, h)) = image::image_dimensions(path) {
                        self.photo_width = w as f32;
                        self.photo_height = h as f32;
                    }
                }

                // Check cache first: project-specific cache folder, then global cache
                let mut faces_opt = None;
                if let Some(session) = studio.session.as_ref() {
                    let proj_cache = Path::new(session.path()).join("cache");
                    faces_opt = concat_vision::FaceCache::load(&proj_cache, path);
                }
                if faces_opt.is_none() {
                    let global_cache = studio.host.dirs.data.join("cache");
                    faces_opt = concat_vision::FaceCache::load(&global_cache, path);
                }

                if let Some(cached_faces) = faces_opt {
                    log::info!("face_zoom: loaded {} face(s) from cache for {:?}", cached_faces.len(), path);
                    self.detected_faces = cached_faces;
                    self.is_cached = true;
                    self.analyzing = false;

                    // If any face was cached and no target was saved, auto-focus on the first one
                    if !had_saved_target {
                        if let Some(first_face) = self.detected_faces.first() {
                            self.selected_face = first_face.id as i32;
                            self.target_x = (first_face.x + first_face.w / 2.0).clamp(0.05, 0.95);
                            self.target_y = (first_face.y + first_face.h / 2.0).clamp(0.05, 0.95);
                        }
                    }
                    return;
                }

                // Not in cache: open immediately and run face detection in background
                self.analyzing = true;
                let path_buf = path.to_path_buf();
                let clip_id_owned = clip_id.to_string();
                let proj_cache_dir = studio.session.as_ref().map(|s| Path::new(s.path()).join("cache"));
                let global_cache_dir = studio.host.dirs.data.join("cache");

                crate::host::spawn_in_project(
                    move || {
                        let mut faces = Vec::new();
                        let mut dimensions = None;
                        if let Ok(dyn_img) = image::open(&path_buf) {
                            let (w, h) = (dyn_img.width(), dyn_img.height());
                            dimensions = Some((w as f32, h as f32));
                            let rgb_img = dyn_img.to_rgb8();
                            if let Ok(detector) = concat_vision::FaceDetector::load() {
                                match detector.detect_rgb(rgb_img.as_raw(), w, h) {
                                    Ok(detected) => {
                                        log::info!(
                                            "face_zoom: detected {} face(s) in background for {:?}",
                                            detected.len(),
                                            path_buf
                                        );
                                        if let Some(proj_cache) = &proj_cache_dir {
                                            concat_vision::FaceCache::save(proj_cache, &path_buf, &detected);
                                        }
                                        concat_vision::FaceCache::save(&global_cache_dir, &path_buf, &detected);
                                        faces = detected;
                                    }
                                    Err(err) => {
                                        log::warn!("face_zoom: background face detection error: {err}");
                                    }
                                }
                            }
                        }
                        (clip_id_owned, faces, dimensions)
                    },
                    |studio, _, _, (clip_id, faces, dimensions)| {
                        studio.handle(crate::panes::Msg::FaceZoom(
                            FaceZoomMsg::AnalysisFinished {
                                clip_id,
                                faces,
                                dimensions,
                            },
                        ));
                    },
                );
                return;
            }
        }

        self.photo = None;
        self.has_photo = false;
        self.analyzing = false;
    }

    fn apply_to_clip(&mut self, studio: &mut Studio) {
        if self.clip_id.is_empty() {
            self.open = false;
            return;
        }

        let clip_id = self.clip_id.clone();
        let zoom = self.zoom as f64;

        // Timeline video resolution (e.g. 1920x1080)
        let (timeline_w, timeline_h) = {
            let active = studio.project().active();
            let w = active.video.width.max(1) as f64;
            let h = active.video.height.max(1) as f64;
            (w, h)
        };

        // Photo dimensions (fallback to timeline if unknown)
        let photo_w = if self.photo_width > 0.0 { self.photo_width as f64 } else { timeline_w };
        let photo_h = if self.photo_height > 0.0 { self.photo_height as f64 } else { timeline_h };

        // Ratio of fitted photo dimensions to timeline dimensions
        let fit = (timeline_w / photo_w).min(timeline_h / photo_h);
        let fitted_w = photo_w * fit;
        let fitted_h = photo_h * fit;

        let scale_x_ratio = fitted_w / timeline_w;
        let scale_y_ratio = fitted_h / timeline_h;

        // Calculate target offset to bring (target_x, target_y) to the center
        // Target coordinates are normalized [0..1] in the picture.
        // Center of picture is (0.5, 0.5).
        // offset_x and offset_y in Concat are fractions of timeline frame size, positive Y moves down.
        let target_offset_x = -(self.target_x as f64 - 0.5) * zoom * scale_x_ratio;
        let target_offset_y = -(self.target_y as f64 - 0.5) * zoom * scale_y_ratio;

        let ease = match self.easing {
            1 => KeyEase::LINEAR,
            2 => KeyEase::IN_OUT,
            _ => KeyEase::OUT, // 0: Cinematic / Ease Out
        };

        // Determine start and end values based on direction
        let (start_scale, start_x, start_y, end_scale, end_x, end_y) = if self.direction == 0 {
            // Zoom In: Full photo (1.0) -> Face focus (zoom, target_offsets)
            (1.0, 0.0, 0.0, zoom, target_offset_x, target_offset_y)
        } else {
            // Zoom Out: Face focus (zoom, target_offsets) -> Full photo (1.0)
            (zoom, target_offset_x, target_offset_y, 1.0, 0.0, 0.0)
        };

        let mut commands = vec![
            // Clear any preexisting user keys on scale and offset
            Command::ClearClipKeys {
                clip_id: clip_id.clone(),
                property: KeyProperty::Scale,
            },
            Command::ClearClipKeys {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
            },
            Command::ClearClipKeys {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetY,
            },
            // Start keyframe at t = 0.0
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Scale,
                at: 0.0,
                value: start_scale,
                ease: KeyEase::LINEAR,
            },
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
                at: 0.0,
                value: start_x,
                ease: KeyEase::LINEAR,
            },
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetY,
                at: 0.0,
                value: start_y,
                ease: KeyEase::LINEAR,
            },
            // End keyframe at t = 1.0 with chosen easing
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Scale,
                at: 1.0,
                value: end_scale,
                ease,
            },
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
                at: 1.0,
                value: end_x,
                ease,
            },
            Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetY,
                at: 1.0,
                value: end_y,
                ease,
            },
        ];

        // Add or update AppliedFilter on the clip's video_effects so it appears in the inspector's "Efectos" tab!
        if let Some(clip) = studio.clip(&clip_id) {
            let mut effects = clip.video_effects.clone();
            let mut filter = AppliedFilter::new("face_zoom");
            filter.params.insert("direction".to_string(), self.direction as f64);
            filter.params.insert("zoom".to_string(), self.zoom as f64);
            filter.params.insert("easing".to_string(), self.easing as f64);
            filter.params.insert("target_x".to_string(), self.target_x as f64);
            filter.params.insert("target_y".to_string(), self.target_y as f64);
            filter.params.insert("face_id".to_string(), self.selected_face as f64);
            filter.enabled = true;

            if let Some(existing_idx) = effects.iter().position(|e| e.id == "face_zoom") {
                effects[existing_idx] = filter;
            } else {
                effects.push(filter);
            }

            commands.push(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    video_effects: Some(effects),
                    ..ClipPatch::default()
                },
            });
        }

        let _ = studio.apply(Command::Batch { commands });
        self.open = false;

        // Switch the right inspector to the Effects tab so the user sees the newly added effect immediately
        studio.inspector_jump = (
            studio.inspector_jump.0 + 1,
            "Effects",
            "Effects",
        );

        let mode_desc = if self.direction == 0 { "Zoom In" } else { "Zoom Out" };
        studio.notify(
            &format!("Efecto {mode_desc} al rostro aplicado con éxito"),
            false,
        );
        studio.request_preview();
    }

    /// Converts pane state into Slint's FaceZoomData.
    pub fn data(&self) -> FaceZoomData {
        let faces_models: Vec<DetectedFaceData> = self
            .detected_faces
            .iter()
            .enumerate()
            .map(|(idx, face)| {
                let face_id = face.id as i32;
                DetectedFaceData {
                    id: face_id,
                    label: format!("Rostro {}", idx + 1).into(),
                    x: face.x,
                    y: face.y,
                    w: face.w,
                    h: face.h,
                    selected: self.selected_face == face_id,
                }
            })
            .collect();

        let has_faces = !faces_models.is_empty();

        FaceZoomData {
            open: self.open,
            clip_id: self.clip_id.clone().into(),
            clip_name: self.clip_name.clone().into(),
            photo: self.photo.clone().unwrap_or_default(),
            has_photo: self.has_photo,
            photo_width: self.photo_width,
            photo_height: self.photo_height,
            target_x: self.target_x,
            target_y: self.target_y,
            zoom: self.zoom,
            direction: self.direction,
            easing: self.easing,
            faces: ModelRc::from(Rc::new(VecModel::from(faces_models))),
            has_faces,
            selected_face: self.selected_face,
            is_cached: self.is_cached,
            analyzing: self.analyzing,
        }
    }
}
