use anyhow::{Context, Result};
use gcode::Mnemonic;
use macroquad::prelude::*;
use std::env;
use std::fs;

#[derive(Clone, Copy, Debug)]
struct Vec3D {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3D {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

#[derive(Clone, Debug)]
struct LineSegment {
    start: Vec3D,
    end: Vec3D,
    is_extrusion: bool,
    layer_z: f32,
}

struct Bounds {
    min: Vec3D,
    max: Vec3D,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min: Vec3D::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3D::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    fn expand(&mut self, p: Vec3D) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }

    fn center(&self) -> Vec3D {
        Vec3D::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    fn max_dimension(&self) -> f32 {
        let dx = self.max.x - self.min.x;
        let dy = self.max.y - self.min.y;
        let dz = self.max.z - self.min.z;
        dx.max(dy).max(dz)
    }
}

struct Camera {
    distance: f32,
    yaw: f32,
    pitch: f32,
    target: Vec3,
}

impl Camera {
    fn new(distance: f32) -> Self {
        Self {
            distance,
            yaw: 45.0_f32.to_radians(),
            pitch: 30.0_f32.to_radians(),
            target: vec3(0.0, 0.0, 0.0),
        }
    }

    fn position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.target + vec3(x, y, z)
    }

    fn reset(&mut self, distance: f32) {
        self.distance = distance;
        self.yaw = 45.0_f32.to_radians();
        self.pitch = 30.0_f32.to_radians();
    }
}

fn parse_gcode(filename: &str) -> Result<Vec<LineSegment>> {
    let content = fs::read_to_string(filename)
        .context(format!("Failed to read file: {}", filename))?;

    let mut segments = Vec::new();
    let mut current_pos = Vec3D::zero();
    let mut e_pos = 0.0_f32;
    let mut absolute_mode = true;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }

        for parsed_line in gcode::parse(trimmed) {
            for gcode in parsed_line.gcodes() {
                match gcode.mnemonic() {
                    Mnemonic::General => {
                        let major = gcode.major_number();
                        if major == 0 || major == 1 {
                            // G0 (rapid) or G1 (linear move)
                            let mut new_pos = current_pos;
                            let mut new_e = e_pos;

                            for arg in gcode.arguments() {
                                match arg.letter {
                                    'X' => new_pos.x = arg.value as f32,
                                    'Y' => new_pos.y = arg.value as f32,
                                    'Z' => new_pos.z = arg.value as f32,
                                    'E' => new_e = arg.value as f32,
                                    _ => {}
                                }
                            }

                            if !absolute_mode {
                                new_pos.x += current_pos.x;
                                new_pos.y += current_pos.y;
                                new_pos.z += current_pos.z;
                                new_e += e_pos;
                            }

                            let is_extrusion = new_e > e_pos;

                            if new_pos.x != current_pos.x || new_pos.y != current_pos.y || new_pos.z != current_pos.z {
                                segments.push(LineSegment {
                                    start: current_pos,
                                    end: new_pos,
                                    is_extrusion,
                                    layer_z: new_pos.z,
                                });
                            }

                            current_pos = new_pos;
                            e_pos = new_e;
                        } else if major == 90 {
                            absolute_mode = true;
                        } else if major == 91 {
                            absolute_mode = false;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(segments)
}

fn filter_priming_lines(segments: &[LineSegment]) -> Vec<LineSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    // Strategy: Find where the actual print starts and ends by looking for
    // clusters of extrusion moves away from edges
    // This skips priming, homing, and positioning moves
    
    let mut start_index = None;
    let mut end_index = None;
    
    // Find first cluster of extrusion moves away from edges
    for (i, window) in segments.windows(5).enumerate() {
        let extrusion_count = window.iter().filter(|s| s.is_extrusion).count();
        
        if extrusion_count >= 3 {
            let away_from_edges = window.iter().all(|s| {
                let at_edge = s.start.x < 10.0 || s.end.x < 10.0 ||
                             s.start.y < 20.0 || s.end.y < 20.0;
                let long_move = (s.end.x - s.start.x).abs() > 100.0 ||
                               (s.end.y - s.start.y).abs() > 100.0;
                !at_edge && !long_move
            });
            
            if away_from_edges {
                start_index = Some(i);
                break;
            }
        }
    }
    
    // Find last extrusion (end of actual print)
    end_index = segments.iter()
        .rposition(|s| s.is_extrusion)
        .map(|i| i + 1); // +1 to include this segment
    
    let start = start_index.unwrap_or(0);
    let end = end_index.unwrap_or(segments.len());
    
    segments[start..end].to_vec()
}

fn compute_bounds(segments: &[LineSegment]) -> Bounds {
    let mut bounds = Bounds::new();
    
    // Only compute bounds from extrusion moves to ignore travel/homing
    for seg in segments {
        if seg.is_extrusion {
            bounds.expand(seg.start);
            bounds.expand(seg.end);
        }
    }
    
    bounds
}

fn window_conf() -> Conf {
    Conf {
        window_title: "GSoda - G-code 3D Viewer".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <gcode-file>", args[0]);
        eprintln!("\nControls:");
        eprintln!("  Mouse drag: Rotate camera");
        eprintln!("  Scroll:     Zoom in/out");
        eprintln!("  R:          Reset camera");
        eprintln!("  L:          Toggle layer filtering");
        eprintln!("  M:          Toggle travel moves");
        eprintln!("  S:          Toggle axis indicator");
        eprintln!("  Up/Down:    Adjust visible layers");
        eprintln!("  Esc:        Quit");
        std::process::exit(1);
    }

    let filename = &args[1];
    println!("Loading G-code file: {}", filename);

    let segments = parse_gcode(filename)?;
    println!("Parsed {} line segments", segments.len());
    
    let segments = filter_priming_lines(&segments);
    println!("After filtering priming: {} segments", segments.len());

    if segments.is_empty() {
        anyhow::bail!("No valid G-code movements found in file");
    }

    let bounds = compute_bounds(&segments);
    let center = bounds.center();
    let scale = 2.0 / bounds.max_dimension();
    let initial_distance = 3.0;

    println!(
        "Bounds: ({:.1}, {:.1}, {:.1}) to ({:.1}, {:.1}, {:.1})",
        bounds.min.x, bounds.min.y, bounds.min.z, bounds.max.x, bounds.max.y, bounds.max.z
    );

    let max_z = bounds.max.z;
    let mut camera = Camera::new(initial_distance);
    let mut layer_filter_enabled = false;
    let mut layer_filter_z = max_z;
    let mut show_travel_moves = true; // Changed to true by default
    let mut show_axis = true; // Show axis indicator by default

    let mut last_mouse_pos: Option<(f32, f32)> = None;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        if is_key_pressed(KeyCode::R) {
            camera.reset(initial_distance);
            println!("Camera reset");
        }

        if is_key_pressed(KeyCode::L) {
            layer_filter_enabled = !layer_filter_enabled;
            println!("Layer filter: {}", if layer_filter_enabled { "ON" } else { "OFF" });
        }

        if is_key_pressed(KeyCode::M) {
            show_travel_moves = !show_travel_moves;
            println!("Travel moves: {}", if show_travel_moves { "ON" } else { "OFF" });
        }

        if is_key_pressed(KeyCode::S) {
            show_axis = !show_axis;
            println!("Axis indicator: {}", if show_axis { "ON" } else { "OFF" });
        }

        if layer_filter_enabled {
            if is_key_pressed(KeyCode::Up) {
                layer_filter_z = (layer_filter_z + 0.5).min(max_z);
                println!("Layer filter Z: {:.2}", layer_filter_z);
            }
            if is_key_pressed(KeyCode::Down) {
                layer_filter_z = (layer_filter_z - 0.5).max(0.0);
                println!("Layer filter Z: {:.2}", layer_filter_z);
            }
        }

        // Mouse rotation
        if is_mouse_button_down(MouseButton::Left) {
            let (mx, my) = mouse_position();
            if let Some((last_x, last_y)) = last_mouse_pos {
                let dx = mx - last_x;
                let dy = my - last_y;
                camera.yaw += dx * 0.01;
                camera.pitch = (camera.pitch - dy * 0.01).clamp(-1.5, 1.5);
            }
            last_mouse_pos = Some((mx, my));
        } else {
            last_mouse_pos = None;
        }

        // Mouse zoom
        let (_, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            camera.distance = (camera.distance - wheel_y * 0.1).max(0.5);
        }

        clear_background(Color::from_rgba(20, 20, 30, 255));

        // Setup 3D camera
        set_camera(&Camera3D {
            position: camera.position(),
            target: camera.target,
            up: vec3(0.0, 1.0, 0.0),
            fovy: 45.0,
            projection: Projection::Perspective,
            ..Default::default()
        });

        // Define light direction (from top-front-right, normalized)
        let light_dir = vec3(0.5, 0.7, 0.3).normalize();

        // Draw toolpath
        for seg in &segments {
            if layer_filter_enabled && seg.layer_z > layer_filter_z {
                continue;
            }

            // Skip travel moves if not enabled
            if !seg.is_extrusion && !show_travel_moves {
                continue;
            }

            let start_scaled = vec3(
                (seg.start.x - center.x) * scale,
                (seg.start.z - center.z) * scale,
                (seg.start.y - center.y) * scale,
            );
            let end_scaled = vec3(
                (seg.end.x - center.x) * scale,
                (seg.end.z - center.z) * scale,
                (seg.end.y - center.y) * scale,
            );

            // Calculate line direction for lighting
            let line_dir = (end_scaled - start_scaled).normalize();
            
            // Simple diffuse lighting: dot product with light direction
            // Use abs to light both sides of the line
            let light_intensity = line_dir.dot(light_dir).abs();
            // Combine with ambient lighting (0.6 base + 0.4 from directional) - brighter overall
            let lighting = 0.6 + light_intensity * 0.4;

            // Calculate color with height-based shading for depth perception
            let height_ratio = (seg.layer_z - bounds.min.z) / (bounds.max.z - bounds.min.z);
            let color = if seg.is_extrusion {
                // Blue extrusion with gradient from dark (bottom) to bright (top)
                let brightness = (0.5 + height_ratio * 0.5) * lighting; // Apply lighting, brighter base
                Color::from_rgba(
                    (100.0 * brightness) as u8,
                    (200.0 * brightness) as u8,
                    (255.0 * brightness) as u8,
                    255
                )
            } else {
                // Red travel moves, slightly dimmed with height
                let brightness = (0.6 + height_ratio * 0.4) * lighting; // Brighter base
                Color::from_rgba(
                    (255.0 * brightness) as u8,
                    (100.0 * brightness) as u8,
                    (100.0 * brightness) as u8,
                    180
                )
            };

            draw_line_3d(start_scaled, end_scaled, color);
        }

        // Draw axis indicator at model corner
        if show_axis {
            let model_size_x = bounds.max.x - bounds.min.x;
            let model_size_y = bounds.max.y - bounds.min.y;
            let model_size_z = bounds.max.z - bounds.min.z;
            
            // Position at bottom-left-front corner of model (in scaled space)
            let axis_origin = vec3(
                (bounds.min.x - center.x) * scale,
                (bounds.min.z - center.z) * scale,
                (bounds.min.y - center.y) * scale,
            );
            
            // Axis lengths match actual model dimensions
            let x_len = model_size_x * scale;
            let y_len = model_size_z * scale;
            let z_len = model_size_y * scale;
            
            // X axis - Red (along model X)
            draw_line_3d(
                axis_origin,
                axis_origin + vec3(x_len, 0.0, 0.0),
                Color::from_rgba(255, 80, 80, 255)
            );
            
            // Y axis (Z in model space) - Green (vertical)
            draw_line_3d(
                axis_origin,
                axis_origin + vec3(0.0, y_len, 0.0),
                Color::from_rgba(80, 255, 80, 255)
            );
            
            // Z axis (Y in model space) - Blue (depth)
            draw_line_3d(
                axis_origin,
                axis_origin + vec3(0.0, 0.0, z_len),
                Color::from_rgba(80, 80, 255, 255)
            );
            
            // Draw tick marks every 10mm (or appropriate interval)
            let max_dim = model_size_x.max(model_size_y).max(model_size_z);
            let tick_interval = if max_dim > 200.0 {
                50.0 // Every 50mm for large models
            } else if max_dim > 100.0 {
                20.0 // Every 20mm for medium models
            } else {
                10.0 // Every 10mm for small models
            };
            
            let tick_size = 0.05; // Size of tick marks in scaled space
            
            // X axis ticks
            let mut x_mm = tick_interval;
            while x_mm <= model_size_x {
                let x_pos = x_mm * scale;
                let tick_pos = axis_origin + vec3(x_pos, 0.0, 0.0);
                draw_line_3d(
                    tick_pos,
                    tick_pos + vec3(0.0, tick_size, 0.0),
                    Color::from_rgba(255, 80, 80, 200)
                );
                // Small cube as marker
                draw_cube(
                    tick_pos + vec3(0.0, tick_size * 1.5, 0.0),
                    vec3(0.015, 0.015, 0.015),
                    None,
                    Color::from_rgba(255, 80, 80, 255)
                );
                x_mm += tick_interval;
            }
            
            // Y axis (vertical) ticks
            let mut y_mm = tick_interval;
            while y_mm <= model_size_z {
                let y_pos = y_mm * scale;
                let tick_pos = axis_origin + vec3(0.0, y_pos, 0.0);
                draw_line_3d(
                    tick_pos,
                    tick_pos + vec3(tick_size, 0.0, 0.0),
                    Color::from_rgba(80, 255, 80, 200)
                );
                draw_cube(
                    tick_pos + vec3(tick_size * 1.5, 0.0, 0.0),
                    vec3(0.015, 0.015, 0.015),
                    None,
                    Color::from_rgba(80, 255, 80, 255)
                );
                y_mm += tick_interval;
            }
            
            // Z axis (depth) ticks
            let mut z_mm = tick_interval;
            while z_mm <= model_size_y {
                let z_pos = z_mm * scale;
                let tick_pos = axis_origin + vec3(0.0, 0.0, z_pos);
                draw_line_3d(
                    tick_pos,
                    tick_pos + vec3(0.0, tick_size, 0.0),
                    Color::from_rgba(80, 80, 255, 200)
                );
                draw_cube(
                    tick_pos + vec3(0.0, tick_size * 1.5, 0.0),
                    vec3(0.015, 0.015, 0.015),
                    None,
                    Color::from_rgba(80, 80, 255, 255)
                );
                z_mm += tick_interval;
            }
            
            // Draw axis labels at the end
            let label_size = vec3(0.03, 0.03, 0.03);
            
            // X label (red) at end of X axis
            draw_cube(
                axis_origin + vec3(x_len + 0.05, 0.0, 0.0),
                label_size,
                None,
                Color::from_rgba(255, 80, 80, 255)
            );
            
            // Y label (green) at end of Y axis
            draw_cube(
                axis_origin + vec3(0.0, y_len + 0.05, 0.0),
                label_size,
                None,
                Color::from_rgba(80, 255, 80, 255)
            );
            
            // Z label (blue) at end of Z axis
            draw_cube(
                axis_origin + vec3(0.0, 0.0, z_len + 0.05),
                label_size,
                None,
                Color::from_rgba(80, 80, 255, 255)
            );
            
            // Draw size label at opposite corner (top-right-back)
            let size_label_pos = vec3(
                (bounds.max.x - center.x) * scale,
                (bounds.max.z - center.z) * scale,
                (bounds.max.y - center.y) * scale,
            );
            
            // Draw a small box to mark the size label location
            draw_cube(
                size_label_pos,
                vec3(0.04, 0.04, 0.04),
                None,
                Color::from_rgba(200, 200, 200, 255)
            );
            
            // Draw lines connecting to show bounding box corner
            let offset = 0.08;
            draw_line_3d(
                size_label_pos,
                size_label_pos + vec3(offset, 0.0, 0.0),
                Color::from_rgba(200, 200, 200, 180)
            );
            draw_line_3d(
                size_label_pos,
                size_label_pos + vec3(0.0, offset, 0.0),
                Color::from_rgba(200, 200, 200, 180)
            );
            draw_line_3d(
                size_label_pos,
                size_label_pos + vec3(0.0, 0.0, offset),
                Color::from_rgba(200, 200, 200, 180)
            );
        }

        // Switch to 2D for UI
        set_default_camera();

        let model_size_x = bounds.max.x - bounds.min.x;
        let model_size_y = bounds.max.y - bounds.min.y;
        let model_size_z = bounds.max.z - bounds.min.z;

        let ui_text = format!(
            "Segments: {} | Size: {:.1}x{:.1}x{:.1}mm | Travel: {} | Axis: {}",
            segments.len(),
            model_size_x,
            model_size_y,
            model_size_z,
            if show_travel_moves { "ON" } else { "OFF" },
            if show_axis { "ON" } else { "OFF" }
        );
        draw_text(&ui_text, 10.0, 25.0, 20.0, WHITE);
        draw_text(
            "Controls: Drag=Rotate | Scroll=Zoom | R=Reset | L=Layers | M=Travel | S=Axis | Up/Down=Filter | Esc=Quit",
            10.0,
            screen_height() - 10.0,
            18.0,
            LIGHTGRAY,
        );

        next_frame().await;
    }

    Ok(())
}
