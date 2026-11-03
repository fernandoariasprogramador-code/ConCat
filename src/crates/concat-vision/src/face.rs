// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Ultra-fast face detection and caching for smart camera framing and Ken Burns effects.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::runtime::{Input, Model};

const FACE_MODEL_BYTES: &[u8] = include_bytes!("../models/face-detector.onnx");

const INPUT_WIDTH: u32 = 320;
const INPUT_HEIGHT: u32 = 240;

/// A detected face with normalized coordinates [0..1].
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectedFace {
    pub id: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub confidence: f32,
}

struct PriorBox {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
}

/// The neural network face detector.
pub struct FaceDetector {
    model: Mutex<Model>,
    priors: Vec<PriorBox>,
}

impl FaceDetector {
    /// Loads the face detector model.
    pub fn load() -> Result<Self, String> {
        let model = Model::from_bytes(FACE_MODEL_BYTES)?;
        let priors = Self::generate_priors();
        Ok(Self {
            model: Mutex::new(model),
            priors,
        })
    }

    /// Generates prior anchor boxes for UltraFace RFB-320 (320x240).
    fn generate_priors() -> Vec<PriorBox> {
        let feature_maps = [(40, 30), (20, 15), (10, 8), (5, 4)];
        let shrinkages = [8.0f32, 16.0, 32.0, 64.0];
        let min_boxes: [&[f32]; 4] = [
            &[10.0, 16.0, 24.0],
            &[32.0, 48.0],
            &[64.0, 96.0],
            &[128.0, 192.0, 256.0],
        ];

        let mut priors = Vec::with_capacity(4420);
        for idx in 0..4 {
            let (fw, fh) = feature_maps[idx];
            let scale_w = 320.0 / shrinkages[idx];
            let scale_h = 240.0 / shrinkages[idx];

            for j in 0..fh {
                for i in 0..fw {
                    let x_center = (i as f32 + 0.5) / scale_w;
                    let y_center = (j as f32 + 0.5) / scale_h;

                    for &min_box in min_boxes[idx] {
                        let w = min_box / 320.0;
                        let h = min_box / 240.0;
                        priors.push(PriorBox {
                            cx: x_center.clamp(0.0, 1.0),
                            cy: y_center.clamp(0.0, 1.0),
                            w: w.clamp(0.0, 1.0),
                            h: h.clamp(0.0, 1.0),
                        });
                    }
                }
            }
        }
        priors
    }

    /// Runs face detection on RGB pixels (width x height, 3 bytes per pixel).
    pub fn detect_rgb(&self, rgb_pixels: &[u8], width: u32, height: u32) -> Result<Vec<DetectedFace>, String> {
        if width == 0 || height == 0 || rgb_pixels.len() < (width * height * 3) as usize {
            return Ok(Vec::new());
        }

        // Resample image to 320x240 and normalize (pixel - 127) / 128
        let mut planes = vec![0.0f32; (3 * INPUT_WIDTH * INPUT_HEIGHT) as usize];
        let in_w = INPUT_WIDTH as usize;
        let in_h = INPUT_HEIGHT as usize;
        let src_w = width as usize;
        let src_h = height as usize;

        for c in 0..3 {
            for y in 0..in_h {
                let sy = (y * src_h / in_h).min(src_h - 1);
                for x in 0..in_w {
                    let sx = (x * src_w / in_w).min(src_w - 1);
                    let val = rgb_pixels[(sy * src_w + sx) * 3 + c] as f32;
                    planes[(c * in_h + y) * in_w + x] = (val - 127.0) / 128.0;
                }
            }
        }

        let input = Input {
            name: "input",
            dims: vec![1, 3, in_h, in_w],
            data: planes.into(),
        };

        let outputs = self
            .model
            .lock()
            .map_err(|_| "Face detector poisoned".to_owned())?
            .run(vec![input], &["scores", "boxes"])?;

        if outputs.len() < 2 {
            return Err("Unexpected output tensor count from face detector".to_owned());
        }

        let scores_data = &outputs[0].data;
        let boxes_data = &outputs[1].data;

        let num_priors = self.priors.len();
        if scores_data.len() < num_priors * 2 || boxes_data.len() < num_priors * 4 {
            return Err("Output tensor shape mismatch".to_owned());
        }

        let mut candidates = Vec::new();
        let confidence_threshold = 0.70f32;

        for i in 0..num_priors {
            let s0 = scores_data[i * 2 + 0];
            let s1 = scores_data[i * 2 + 1];

            // Calculate face probability (support both pre-softmax and post-softmax)
            let conf = if (s0 + s1 - 1.0).abs() < 0.05 {
                s1
            } else {
                1.0 / (1.0 + (s0 - s1).exp())
            };

            if conf > confidence_threshold {
                let prior = &self.priors[i];
                let cx = boxes_data[i * 4 + 0] * 0.1 * prior.w + prior.cx;
                let cy = boxes_data[i * 4 + 1] * 0.1 * prior.h + prior.cy;
                let w = (boxes_data[i * 4 + 2] * 0.2).exp() * prior.w;
                let h = (boxes_data[i * 4 + 3] * 0.2).exp() * prior.h;

                let x = (cx - w / 2.0).clamp(0.0, 1.0);
                let y = (cy - h / 2.0).clamp(0.0, 1.0);
                let w = w.clamp(0.02, 1.0 - x);
                let h = h.clamp(0.02, 1.0 - y);

                candidates.push(DetectedFace {
                    id: 0,
                    x,
                    y,
                    w,
                    h,
                    confidence: conf,
                });
            }
        }

        Ok(Self::suppress_duplicates(candidates))
    }

    /// Determines if two face bounding boxes correspond to the same person.
    pub fn is_same_face(a: &DetectedFace, b: &DetectedFace) -> bool {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = (a.x + a.w).min(b.x + b.w);
        let y2 = (a.y + a.h).min(b.y + b.h);

        let inter_w = (x2 - x1).max(0.0);
        let inter_h = (y2 - y1).max(0.0);
        let inter_area = inter_w * inter_h;

        if inter_area <= 0.0 {
            return false;
        }

        let area_a = a.w * a.h;
        let area_b = b.w * b.h;
        let min_area = area_a.min(area_b);
        let union_area = area_a + area_b - inter_area;

        let iou = if union_area > 0.0 { inter_area / union_area } else { 0.0 };
        let iom = if min_area > 0.0 { inter_area / min_area } else { 0.0 };

        let ca_x = a.x + a.w / 2.0;
        let ca_y = a.y + a.h / 2.0;
        let cb_x = b.x + b.w / 2.0;
        let cb_y = b.y + b.h / 2.0;
        let center_dist = ((ca_x - cb_x).powi(2) + (ca_y - cb_y).powi(2)).sqrt();
        let min_span = a.w.min(b.w).min(a.h.min(b.h));

        iou > 0.25 || iom > 0.40 || center_dist < (min_span * 0.6)
    }

    /// Filters overlapping face detections and sorts them left-to-right.
    pub fn suppress_duplicates(mut faces: Vec<DetectedFace>) -> Vec<DetectedFace> {
        faces.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        let mut result = Vec::new();

        for face in faces {
            let mut overlaps = false;
            for kept in &result {
                if Self::is_same_face(&face, kept) {
                    overlaps = true;
                    break;
                }
            }
            if !overlaps {
                result.push(face);
            }
        }

        // Sort faces left-to-right for consistent numbering (Rostro 1, Rostro 2, ...)
        result.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        for (idx, face) in result.iter_mut().enumerate() {
            face.id = idx;
        }

        result
    }
}

/// Cache management for storing and retrieving face detection results.
pub struct FaceCache;

impl FaceCache {
    /// Generates a unique cache file path for the media file inside `cache_dir`.
    pub fn cache_path(cache_dir: &Path, media_path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        b"v2_faces".hash(&mut hasher);
        media_path.to_string_lossy().hash(&mut hasher);
        if let Ok(metadata) = std::fs::metadata(media_path) {
            if let Ok(modified) = metadata.modified() {
                modified.hash(&mut hasher);
            }
            metadata.len().hash(&mut hasher);
        }
        let hash = hasher.finish();
        cache_dir.join("faces").join(format!("{hash:016x}.json"))
    }

    /// Reads cached faces if available, consolidating duplicates.
    pub fn load(cache_dir: &Path, media_path: &Path) -> Option<Vec<DetectedFace>> {
        let path = Self::cache_path(cache_dir, media_path);
        if path.is_file() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(faces) = serde_json::from_str::<Vec<DetectedFace>>(&data) {
                    let cleaned = FaceDetector::suppress_duplicates(faces);
                    return Some(cleaned);
                }
            }
        }
        None
    }

    /// Saves detected faces to the cache.
    pub fn save(cache_dir: &Path, media_path: &Path, faces: &[DetectedFace]) {
        let path = Self::cache_path(cache_dir, media_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string(faces) {
            let _ = std::fs::write(&path, data);
        }
    }
}
