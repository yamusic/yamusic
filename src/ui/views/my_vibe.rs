use flume::{Receiver, Sender};
use lazy_static::lazy_static;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect, style::Color};
use std::thread;
use std::time::SystemTime;

use crate::{
    ui::{
        context::{AppContext, GlobalUiState},
        traits::{Action, Component},
    },
    util::colors,
};

lazy_static! {
    static ref PERMUTATION: [u8; 512] = {
        let mut p = [0u8; 512];
        let mut seed: u32 = 0xDEADBEEF;
        for i in 0..256 {
            p[i] = i as u8;
        }
        for i in (1..256).rev() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let j = (seed as usize) % (i + 1);
            p.swap(i, j);
        }
        for i in 0..256 {
            p[i + 256] = p[i];
        }
        p
    };
    static ref GRADIENTS3: [[f32; 3]; 12] = [
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [-1.0, 0.0, -1.0],
        [0.0, 1.0, 1.0],
        [0.0, -1.0, 1.0],
        [0.0, 1.0, -1.0],
        [0.0, -1.0, -1.0]
    ];
}

#[derive(Clone, Copy, Debug)]
struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vector3 {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

fn hex_to_vector3(hex: &str) -> Vector3 {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        Vector3::new(r, g, b)
    } else {
        Vector3::new(0.5, 0.5, 0.5)
    }
}

fn shift_color(v: Vector3) -> Vector3 {
    let brightness = (v.x + v.y + v.z) / 3.0;
    if brightness < 0.5 {
        Vector3::new(
            (v.x + 0.2).min(1.0),
            (v.y + 0.2).min(1.0),
            (v.z + 0.2).min(1.0),
        )
    } else {
        Vector3::new(v.x * 0.8, v.y * 0.8, v.z * 0.8)
    }
}

const DEFAULT_PALETTE: [Vector3; 6] = [
    Vector3 {
        x: 0.9,
        y: 0.1,
        z: 0.5,
    },
    Vector3 {
        x: 0.1,
        y: 0.5,
        z: 0.9,
    },
    Vector3 {
        x: 0.9,
        y: 0.8,
        z: 0.1,
    },
    Vector3 {
        x: 0.5,
        y: 0.1,
        z: 0.9,
    },
    Vector3 {
        x: 0.1,
        y: 0.9,
        z: 0.5,
    },
    Vector3 {
        x: 0.9,
        y: 0.5,
        z: 0.1,
    },
];

#[inline(always)]
fn fast_floor(x: f32) -> i32 {
    if x >= 0.0 { x as i32 } else { (x as i32) - 1 }
}

#[inline(always)]
fn dot(g: &[f32; 3], x: f32, y: f32, z: f32) -> f32 {
    g[0] * x + g[1] * y + g[2] * z
}
fn simplex_noise(x: f32, y: f32, z: f32) -> f32 {
    let f3 = 1.0 / 3.0;
    let g3 = 1.0 / 6.0;

    let s = (x + y + z) * f3;
    let i = fast_floor(x + s);
    let j = fast_floor(y + s);
    let k = fast_floor(z + s);

    let t = (i + j + k) as f32 * g3;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);
    let z0 = z - (k as f32 - t);

    let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
        if y0 >= z0 {
            (1, 0, 0, 1, 1, 0)
        }
        // X Y Z
        else if x0 >= z0 {
            (1, 0, 0, 1, 0, 1)
        }
        // X Z Y
        else {
            (0, 0, 1, 1, 0, 1)
        } // Z X Y
    } else {
        if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        }
        // Z Y X
        else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        }
        // Y Z X
        else {
            (0, 1, 0, 1, 1, 0)
        } // Y X Z
    };

    let x1 = x0 - i1 as f32 + g3;
    let y1 = y0 - j1 as f32 + g3;
    let z1 = z0 - k1 as f32 + g3;
    let x2 = x0 - i2 as f32 + 2.0 * g3;
    let y2 = y0 - j2 as f32 + 2.0 * g3;
    let z2 = z0 - k2 as f32 + 2.0 * g3;
    let x3 = x0 - 1.0 + 3.0 * g3;
    let y3 = y0 - 1.0 + 3.0 * g3;
    let z3 = z0 - 1.0 + 3.0 * g3;

    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;

    let gi0 = PERMUTATION
        [(ii + PERMUTATION[(jj + PERMUTATION[kk as usize] as i32) as usize] as i32) as usize]
        as usize
        % 12;
    let gi1 = PERMUTATION[(ii
        + i1
        + PERMUTATION[(jj + j1 + PERMUTATION[(kk + k1) as usize] as i32) as usize] as i32)
        as usize] as usize
        % 12;
    let gi2 = PERMUTATION[(ii
        + i2
        + PERMUTATION[(jj + j2 + PERMUTATION[(kk + k2) as usize] as i32) as usize] as i32)
        as usize] as usize
        % 12;
    let gi3 = PERMUTATION[(ii
        + 1
        + PERMUTATION[(jj + 1 + PERMUTATION[(kk + 1) as usize] as i32) as usize] as i32)
        as usize] as usize
        % 12;

    let mut n0 = 0.0;
    let mut n1 = 0.0;
    let mut n2 = 0.0;
    let mut n3 = 0.0;

    let t0 = 0.6 - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 > 0.0 {
        let t = t0 * t0;
        n0 = t * t * dot(&GRADIENTS3[gi0], x0, y0, z0);
    }

    let t1 = 0.6 - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 > 0.0 {
        let t = t1 * t1;
        n1 = t * t * dot(&GRADIENTS3[gi1], x1, y1, z1);
    }

    let t2 = 0.6 - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 > 0.0 {
        let t = t2 * t2;
        n2 = t * t * dot(&GRADIENTS3[gi2], x2, y2, z2);
    }

    let t3 = 0.6 - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 > 0.0 {
        let t = t3 * t3;
        n3 = t * t * dot(&GRADIENTS3[gi3], x3, y3, z3);
    }

    32.0 * (n0 + n1 + n2 + n3)
}

#[inline(always)]
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub struct MyVibe {
    last_tick: SystemTime,
    phase: f32,
    smoothed_amplitude: f32,
    tx: Sender<RenderRequest>,
    rx: Receiver<RenderResult>,
    last_frame: Option<RenderResult>,
    start_palette: [Vector3; 6],
    target_palette: [Vector3; 6],
    transition_progress: f32,
    last_track_id: Option<String>,
}

impl Default for MyVibe {
    fn default() -> Self {
        let (tx, rx_req) = flume::bounded::<RenderRequest>(1);
        let (tx_res, rx) = flume::unbounded::<RenderResult>();

        thread::spawn(move || {
            while let Ok(req) = rx_req.recv() {
                let result = render_frame(req);
                if tx_res.send(result).is_err() {
                    break;
                }
            }
        });

        Self {
            last_tick: SystemTime::now(),
            phase: 0.0,
            smoothed_amplitude: 0.0,
            tx,
            rx,
            last_frame: None,
            start_palette: DEFAULT_PALETTE,
            target_palette: DEFAULT_PALETTE,
            transition_progress: 1.0,
            last_track_id: None,
        }
    }
}

impl Component for MyVibe {
    fn render(&mut self, f: &mut Frame, area: Rect, ctx: &AppContext, _state: &GlobalUiState) {
        let now = SystemTime::now();
        let dt = now
            .duration_since(self.last_tick)
            .unwrap_or_default()
            .as_secs_f32();
        self.last_tick = now;

        let amplitude = ctx.audio_system.current_amplitude();
        let target_amp = (amplitude * 1.5).min(1.0);

        if target_amp > self.smoothed_amplitude {
            self.smoothed_amplitude = self.smoothed_amplitude * 0.85 + target_amp * 0.15;
        } else {
            self.smoothed_amplitude = self.smoothed_amplitude * 0.95 + target_amp * 0.05;
        }
        let speed = 0.8 + self.smoothed_amplitude * 2.0;
        self.phase += dt * speed;

        let current_track = ctx.audio_system.current_track();
        let track_id = current_track.as_ref().map(|t| t.id.clone());

        if track_id != self.last_track_id {
            self.last_track_id = track_id;

            for i in 0..6 {
                self.start_palette[i] =
                    self.start_palette[i].lerp(self.target_palette[i], self.transition_progress);
            }

            if let Some(track) = current_track {
                if let Some(colors) = &track.derived_colors {
                    let avg = hex_to_vector3(&colors.average);
                    let accent = hex_to_vector3(&colors.accent);
                    let wave = hex_to_vector3(&colors.wave_text);

                    let avg_sec = shift_color(avg);
                    let accent_sec = shift_color(accent);
                    let wave_sec = shift_color(wave);

                    self.target_palette = [avg, accent, wave, avg_sec, accent_sec, wave_sec];
                } else {
                    self.target_palette = DEFAULT_PALETTE;
                }
            } else {
                self.target_palette = DEFAULT_PALETTE;
            }

            self.transition_progress = 0.0;
        }

        self.transition_progress = (self.transition_progress + dt * 0.5).min(1.0);

        let mut current_palette = [Vector3::new(0.0, 0.0, 0.0); 6];
        for i in 0..6 {
            current_palette[i] =
                self.start_palette[i].lerp(self.target_palette[i], self.transition_progress);
        }

        let inner_area = area;
        let width = inner_area.width as usize;
        let height = inner_area.height as usize;

        while let Ok(res) = self.rx.try_recv() {
            self.last_frame = Some(res);
        }

        let (bg_r, bg_g, bg_b) = match colors::BACKGROUND {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        };

        let req = RenderRequest {
            width,
            height,
            time: self.phase,
            amplitude: self.smoothed_amplitude,
            bg_rgb: (bg_r, bg_g, bg_b),
            palette: current_palette,
        };
        let _ = self.tx.try_send(req);

        if let Some(frame) = &self.last_frame {
            if frame.width == width && frame.height == height {
                let buf = f.buffer_mut();
                for y in 0..height {
                    for x in 0..width {
                        let idx = y * width + x;
                        if idx < frame.data.len() {
                            let ((r_top, g_top, b_top), (r_bot, g_bot, b_bot)) = frame.data[idx];

                            if let Some(cell) = buf.cell_mut((
                                inner_area.left() + x as u16,
                                inner_area.top() + y as u16,
                            )) {
                                cell.set_char('▀')
                                    .set_fg(Color::Rgb(r_top, g_top, b_top))
                                    .set_bg(Color::Rgb(r_bot, g_bot, b_bot));
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_input(
        &mut self,
        _key: KeyEvent,
        _ctx: &AppContext,
        _state: &GlobalUiState,
    ) -> Option<Action> {
        None
    }
}

struct RenderRequest {
    width: usize,
    height: usize,
    time: f32,
    amplitude: f32,
    bg_rgb: (u8, u8, u8),
    palette: [Vector3; 6],
}

struct RenderResult {
    width: usize,
    height: usize,
    data: Vec<((u8, u8, u8), (u8, u8, u8))>,
}

fn calculate_noise_shape(
    uv: Vector3,
    color1: Vector3,
    color2: Vector3,
    strength: f32,
    offset: f32,
    time: f32,
) -> (Vector3, f32) {
    let len = (uv.x * uv.x + uv.y * uv.y).sqrt();
    if len > 1.5 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let n0 = simplex_noise(
        uv.x * 1.2 + offset,
        uv.y * 1.2 + offset,
        time * 0.5 + offset,
    ) * 0.5
        + 0.5;
    let r0 = n0;

    let d0 = (len - r0).abs();

    let edge0 = r0 + 0.05 + (time.sin() * 0.2 + 0.25);
    let v0 = smooth_step(edge0, r0, len);

    let intensity =
        0.05 * (1.0 + 0.35 * (-(time * 1.5 + offset * 0.35).sin() * 0.5)) + 0.08 * strength;
    let v1 = intensity / (1.0 + d0 + d0 * 70.0);

    if v0 < 0.05 && v1 < 0.02 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let mix_factor = (uv.y * 2.0).clamp(0.0, 1.0);
    let mut col = color1.lerp(color2, mix_factor);

    col.x = (col.x + v1).clamp(0.0, 1.0);
    col.y = (col.y + v1).clamp(0.0, 1.0);
    col.z = (col.z + v1).clamp(0.0, 1.0);

    (col, v0)
}

fn calculate_blob_layer(
    uv: Vector3,
    blob_radius_param: f32,
    color1: Vector3,
    color2: Vector3,
    width: f32,
    base_reaction: f32,
    like_reaction: f32,
    audio_strength: f32,
    offset: f32,
    time: f32,
) -> (Vector3, f32) {
    let len = (uv.x * uv.x + uv.y * uv.y).sqrt();
    let subtle_reaction = (like_reaction * 0.3 + audio_strength * 1.2).min(0.95);
    let outer_radius =
        blob_radius_param + width * 0.5 + base_reaction * (1.0 + subtle_reaction * 1.5);

    if len > outer_radius + 0.6 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let strength = (like_reaction * 0.7 + audio_strength * 0.45).min(0.65);

    let scale = 1.0 - like_reaction * 0.5;
    let (mut noise_col, mut noise_alpha) = calculate_noise_shape(
        Vector3::new(uv.x * scale, uv.y * scale, 0.0),
        color1,
        color2,
        strength,
        offset,
        time,
    );

    let alpha_falloff = smooth_step(outer_radius + 0.4, 0.22, len);
    noise_alpha = lerp(0.0, noise_alpha, alpha_falloff);

    if noise_alpha < 0.03 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let highlight = 0.6 * like_reaction * (1.0 - smooth_step(0.2, outer_radius * 0.8, len));
    noise_col.x += highlight;
    noise_col.y += highlight;
    noise_col.z += highlight;

    (noise_col, noise_alpha)
}

fn process_pixel(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    bg_color: Vector3,
    bg_rgb: (u8, u8, u8),
    time: f32,
    amplitude: f32,
    palette: &[Vector3; 6],
) -> ((u8, u8, u8), (u8, u8, u8)) {
    let aspect = (width as f32) / (height as f32 * 2.0);
    let u = (x as f32 / width as f32) * 2.0 - 1.0;
    let u_scaled = u * aspect * 1.6;
    if u_scaled.abs() > 2.5 {
        return (bg_rgb, bg_rgb);
    }

    let mut top_color = bg_color;
    let mut bot_color = bg_color;
    let mut top_alpha_acc = 0.0;
    let mut bot_alpha_acc = 0.0;
    let v_top = ((y as f32 * 2.0) / (height as f32 * 2.0)) * 2.0 - 1.0;
    let v_top_scaled = v_top * 1.6;
    let v_bot = ((y as f32 * 2.0 + 1.0) / (height as f32 * 2.0)) * 2.0 - 1.0;
    let v_bot_scaled = v_bot * 1.6;
    let n0_top = simplex_noise(u_scaled * 1.2, v_top_scaled * 1.2, time * 0.5) * 0.5 + 0.5;
    let n0_bot = simplex_noise(u_scaled * 1.2, v_bot_scaled * 1.2, time * 0.5) * 0.5 + 0.5;
    for i in 0..3 {
        let fi = i as f32;
        let radius = 0.6 - 0.12 * fi;
        let width = 0.5 - 0.15 * fi;
        let spark = 1.0 - 0.2 * fi;
        let offset = 0.0 + 1.57 * fi;
        let blob_param_top = lerp(radius, radius + 0.15, n0_top);
        let (col, alpha) = calculate_blob_layer(
            Vector3::new(u_scaled, v_top_scaled, 0.0),
            blob_param_top,
            palette[i],
            palette[i + 3],
            width,
            spark * 0.3,
            amplitude * 0.12,
            amplitude * 0.65,
            offset,
            time,
        );
        top_color = top_color.lerp(col, alpha);
        top_alpha_acc += alpha;
        let blob_param_bot = lerp(radius, radius + 0.15, n0_bot);
        let (col, alpha) = calculate_blob_layer(
            Vector3::new(u_scaled, v_bot_scaled, 0.0),
            blob_param_bot,
            palette[i],
            palette[i + 3],
            width,
            spark * 0.3,
            amplitude * 0.12,
            amplitude * 0.65,
            offset,
            time,
        );
        bot_color = bot_color.lerp(col, alpha);
        bot_alpha_acc += alpha;
    }

    let r_top = if top_alpha_acc >= 0.01 {
        (top_color.x.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.0
    };
    let g_top = if top_alpha_acc >= 0.01 {
        (top_color.y.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.1
    };
    let b_top = if top_alpha_acc >= 0.01 {
        (top_color.z.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.2
    };

    let r_bot = if bot_alpha_acc >= 0.01 {
        (bot_color.x.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.0
    };
    let g_bot = if bot_alpha_acc >= 0.01 {
        (bot_color.y.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.1
    };
    let b_bot = if bot_alpha_acc >= 0.01 {
        (bot_color.z.clamp(0.0, 1.0) * 255.0) as u8
    } else {
        bg_rgb.2
    };

    ((r_top, g_top, b_top), (r_bot, g_bot, b_bot))
}

fn render_frame(req: RenderRequest) -> RenderResult {
    let width = req.width;
    let height = req.height;
    let time = req.time;
    let amplitude = req.amplitude;
    let (bg_r, bg_g, bg_b) = req.bg_rgb;
    let bg_vec = Vector3::new(
        bg_r as f32 / 255.0,
        bg_g as f32 / 255.0,
        bg_b as f32 / 255.0,
    );

    let palette = req.palette;

    let mut data = vec![((0, 0, 0), (0, 0, 0)); width * height];

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_height = (height + num_threads - 1) / num_threads;

    std::thread::scope(|s| {
        for (i, chunk) in data.chunks_mut(width * chunk_height).enumerate() {
            let start_y = i * chunk_height;
            let palette_ref = &palette;

            s.spawn(move || {
                for (local_y, row) in chunk.chunks_mut(width).enumerate() {
                    let y = start_y + local_y;
                    if y >= height {
                        break;
                    }

                    for (x, pixel) in row.iter_mut().enumerate() {
                        *pixel = process_pixel(
                            x,
                            y,
                            width,
                            height,
                            bg_vec,
                            (bg_r, bg_g, bg_b),
                            time,
                            amplitude,
                            palette_ref,
                        );
                    }
                }
            });
        }
    });

    RenderResult {
        width,
        height,
        data,
    }
}
