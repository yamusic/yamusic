use async_trait::async_trait;
use flume::{Receiver, Sender};
use lazy_static::lazy_static;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use rayon::prelude::*;
use std::thread;
use std::time::Instant;
use yandex_music::model::landing::wave::LandingWave;

use crate::{
    event::events::Event,
    ui::{
        context::AppContext,
        state::AppState,
        traits::{Action, View},
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
    static ref GRADIENTS_TABLE: [[f32; 3]; 512] = {
        let mut table = [[0.0; 3]; 512];
        for i in 0..512 {
            table[i] = GRADIENTS3[PERMUTATION[i] as usize % 12];
        }
        table
    };
}

#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    #[inline(always)]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub fn lerp(self, other: Self, t: f32) -> Self {
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

    let idx0 = (ii + PERMUTATION[(jj + PERMUTATION[kk as usize] as i32) as usize] as i32) as usize;
    let idx1 =
        (ii + i1 + PERMUTATION[(jj + j1 + PERMUTATION[(kk + k1) as usize] as i32) as usize] as i32)
            as usize;
    let idx2 =
        (ii + i2 + PERMUTATION[(jj + j2 + PERMUTATION[(kk + k2) as usize] as i32) as usize] as i32)
            as usize;
    let idx3 =
        (ii + 1 + PERMUTATION[(jj + 1 + PERMUTATION[(kk + 1) as usize] as i32) as usize] as i32)
            as usize;

    let mut n0 = 0.0;
    let mut n1 = 0.0;
    let mut n2 = 0.0;
    let mut n3 = 0.0;

    let t0 = 0.6 - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 > 0.0 {
        let t = t0 * t0;
        n0 = t * t * dot(&GRADIENTS_TABLE[idx0], x0, y0, z0);
    }

    let t1 = 0.6 - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 > 0.0 {
        let t = t1 * t1;
        n1 = t * t * dot(&GRADIENTS_TABLE[idx1], x1, y1, z1);
    }

    let t2 = 0.6 - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 > 0.0 {
        let t = t2 * t2;
        n2 = t * t * dot(&GRADIENTS_TABLE[idx2], x2, y2, z2);
    }

    let t3 = 0.6 - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 > 0.0 {
        let t = t3 * t3;
        n3 = t * t * dot(&GRADIENTS_TABLE[idx3], x3, y3, z3);
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

pub struct MyWave {
    last_tick: Instant,
    phase: f32,
    smoothed_amplitude: f32,
    bass_envelope: f32,
    fade: f32,
    fade_target: f32,
    tx: Sender<RenderRequest>,
    rx: Receiver<RenderResult>,
    front_buffer: Option<RenderResult>,
    back_buffer: Option<RenderResult>,
    start_palette: [Vector3; 6],
    target_palette: [Vector3; 6],
    transition_progress: f32,
    last_track_id: Option<String>,

    waves: Vec<LandingWave>,
    selections: Vec<Option<usize>>,
    show_settings: bool,
    loading: bool,
    focused_index: usize,
    dropdown_open: bool,
    dropdown_selection_index: usize,
    dropdown_state: ListState,
    wave_rx: Receiver<Vec<LandingWave>>,
    wave_tx: Sender<Vec<LandingWave>>,
    pending_request: bool,
}

impl Default for MyWave {
    fn default() -> Self {
        let (tx, rx_req) = flume::unbounded::<RenderRequest>();
        let (tx_res, rx) = flume::unbounded::<RenderResult>();
        let (wave_tx, wave_rx) = flume::unbounded();

        thread::Builder::new()
            .name("wave-renderer".to_string())
            .spawn(move || {
                let mut gpu_renderer = crate::ui::views::my_wave_gpu::GpuRenderer::new_blocking();

                while let Ok(req) = rx_req.recv() {
                    let result = if let Some(renderer) = &mut gpu_renderer {
                        renderer.render(req)
                    } else {
                        render_frame(req)
                    };
                    if tx_res.send(result).is_err() {
                        break;
                    }
                }
            })
            .expect("Failed to spawn renderer thread");

        Self {
            last_tick: Instant::now(),
            phase: 0.0,
            smoothed_amplitude: 0.0,
            bass_envelope: 0.0,

            fade: 0.0,
            fade_target: 0.0,
            tx,
            rx,
            front_buffer: None,
            back_buffer: None,
            start_palette: DEFAULT_PALETTE,
            target_palette: DEFAULT_PALETTE,
            transition_progress: 1.0,
            last_track_id: None,

            waves: Vec::new(),
            selections: Vec::new(),
            show_settings: false,
            loading: false,
            focused_index: 0,
            dropdown_open: false,
            dropdown_selection_index: 0,
            dropdown_state: ListState::default(),
            wave_rx,
            wave_tx,
            pending_request: false,
        }
    }
}

#[async_trait]
impl View for MyWave {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, ctx: &AppContext) {
        if self.fade_target == 0.0 && self.fade == 0.0 && !self.show_settings {
            self.fade_target = 1.0;
        }
        if let Ok(waves) = self.wave_rx.try_recv() {
            self.waves = waves;
            self.selections = vec![None; self.waves.len()];
            self.loading = false;
        }

        if self.waves.is_empty() && !self.loading {
            self.loading = true;
            let api = ctx.api.clone();
            let tx = self.wave_tx.clone();
            tokio::spawn(async move {
                if let Ok(waves) = api.fetch_waves().await {
                    let _ = tx.send(waves);
                }
            });
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        let amplitude = ctx.audio_system.current_amplitude();
        let target_amp = (amplitude * 1.5).min(1.0);

        if target_amp > self.smoothed_amplitude {
            self.smoothed_amplitude = self.smoothed_amplitude * 0.85 + target_amp * 0.15;
        } else {
            self.smoothed_amplitude = self.smoothed_amplitude * 0.95 + target_amp * 0.05;
        }

        if target_amp > self.bass_envelope {
            self.bass_envelope = self.bass_envelope * 0.3 + target_amp * 0.7;
        } else {
            self.bass_envelope = self.bass_envelope * 0.92 + target_amp * 0.08;
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

        if let Ok(res) = self.rx.try_recv() {
            self.back_buffer = self.front_buffer.take();
            self.front_buffer = Some(res);
            self.pending_request = false;
        }

        let (bg_r, bg_g, bg_b) = match colors::BACKGROUND {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        };

        let fade_speed = 3.0;
        let fade_delta = dt * fade_speed;
        if self.fade < self.fade_target {
            self.fade = (self.fade + fade_delta).min(self.fade_target);
        } else if self.fade > self.fade_target {
            self.fade = (self.fade - fade_delta).max(self.fade_target);
        }

        if !self.pending_request {
            let buffer = self.back_buffer.take().map(|b| b.data).unwrap_or_default();
            let req = RenderRequest {
                width,
                height,
                time: self.phase,
                amplitude: self.smoothed_amplitude,
                bg_rgb: (bg_r, bg_g, bg_b),
                palette: current_palette,
                buffer,
            };
            if self.tx.send(req).is_ok() {
                self.pending_request = true;
            }
        }

        if let Some(frame) = &self.front_buffer {
            if frame.width == width && frame.height == height {
                let buf = f.buffer_mut();

                let extra_dim = if self.show_settings { 0.2 } else { 1.0 };
                for y in 0..height {
                    for x in 0..width {
                        let idx = y * width + x;
                        if idx < frame.data.len() {
                            let ((r_top, g_top, b_top), (r_bot, g_bot, b_bot)) = frame.data[idx];

                            let fade = (self.fade * extra_dim).clamp(0.0, 1.0);
                            let r_top = lerp(bg_r as f32, r_top as f32, fade) as u8;
                            let g_top = lerp(bg_g as f32, g_top as f32, fade) as u8;
                            let b_top = lerp(bg_b as f32, b_top as f32, fade) as u8;
                            let r_bot = lerp(bg_r as f32, r_bot as f32, fade) as u8;
                            let g_bot = lerp(bg_g as f32, g_bot as f32, fade) as u8;
                            let b_bot = lerp(bg_b as f32, b_bot as f32, fade) as u8;

                            if let Some(cell) = buf.cell_mut((
                                inner_area.left() + x as u16,
                                inner_area.top() + y as u16,
                            )) {
                                if self.show_settings {
                                    let r_avg = ((r_top as u16 + r_bot as u16) / 2) as u8;
                                    let g_avg = ((g_top as u16 + g_bot as u16) / 2) as u8;
                                    let b_avg = ((b_top as u16 + b_bot as u16) / 2) as u8;
                                    cell.set_char(' ').set_bg(Color::Rgb(r_avg, g_avg, b_avg));
                                } else {
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

        if self.show_settings {
            self.render_settings(f, area);
        }
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let mut seeds = Vec::new();
            for (i, &sel_idx) in self.selections.iter().enumerate() {
                if let Some(idx) = sel_idx {
                    if let Some(item) = self.waves[i].items.get(idx) {
                        seeds.extend(item.seeds.clone());
                    }
                }
            }

            let api = ctx.api.clone();
            let tx = ctx.event_tx.clone();

            tokio::spawn(async move {
                if let Ok(session) = api.create_session(seeds).await {
                    let session_tracks = session.sequence.iter().map(|s| s.track.clone()).collect();
                    let _ = tx.send(Event::WaveReady(session, session_tracks));
                }
            });

            self.show_settings = false;
            return Some(Action::None);
        }

        if key.code == KeyCode::Char('s') && !self.dropdown_open {
            self.show_settings = !self.show_settings;
            if !self.show_settings {
                self.fade_target = 1.0;
            }
            return Some(Action::None);
        }

        if self.show_settings {
            match key.code {
                KeyCode::Char('R') => {
                    for sel in self.selections.iter_mut() {
                        *sel = None;
                    }
                    self.dropdown_open = false;
                }
                KeyCode::Char('r') => {
                    self.selections[self.focused_index] = None;
                    self.dropdown_open = false;
                }
                _ => {}
            }
            if self.dropdown_open {
                match key.code {
                    KeyCode::Up => {
                        if self.dropdown_selection_index > 0 {
                            self.dropdown_selection_index -= 1;
                            self.dropdown_state
                                .select(Some(self.dropdown_selection_index));
                        }
                    }
                    KeyCode::Down => {
                        if let Some(wave) = self.waves.get(self.focused_index) {
                            // +1 for empty selection
                            if self.dropdown_selection_index < wave.items.len() {
                                self.dropdown_selection_index += 1;
                                self.dropdown_state
                                    .select(Some(self.dropdown_selection_index));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if self.dropdown_selection_index == 0 {
                            self.selections[self.focused_index] = None;
                        } else {
                            self.selections[self.focused_index] =
                                Some(self.dropdown_selection_index - 1);
                        }
                        self.dropdown_open = false;
                    }
                    KeyCode::Esc => {
                        self.dropdown_open = false;
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Up => {
                        if self.focused_index > 0 {
                            self.focused_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if self.focused_index < self.waves.len().saturating_sub(1) {
                            self.focused_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if self.focused_index < self.waves.len() {
                            self.dropdown_open = true;
                            self.dropdown_selection_index = self.selections[self.focused_index]
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            self.dropdown_state
                                .select(Some(self.dropdown_selection_index));
                        }
                    }
                    KeyCode::Esc => {
                        self.show_settings = false;
                        self.fade_target = 1.0;
                    }
                    _ => {}
                }
            }
            return Some(Action::None);
        }
        None
    }
}

impl MyWave {
    fn render_settings(&mut self, f: &mut Frame, area: Rect) {
        let overlay_area = centered_rect(area, 60, 80);
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(" My Wave Settings "),
            overlay_area,
        );

        if self.loading {
            f.render_widget(
                Paragraph::new("Loading waves...").alignment(Alignment::Center),
                overlay_area,
            );
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .margin(1)
            .split(overlay_area);

        let content_area = chunks[0];

        let mut constraints = Vec::new();
        for _ in 0..self.waves.len() {
            constraints.push(Constraint::Length(3));
        }
        constraints.push(Constraint::Min(0));

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .margin(1)
            .split(content_area);

        for (i, wave) in self.waves.iter().enumerate() {
            let is_focused = i == self.focused_index;
            let selected_text = if let Some(idx) = self.selections[i] {
                wave.items
                    .get(idx)
                    .map(|item| item.title.clone())
                    .unwrap_or_else(|| "None".to_string())
            } else {
                "None".to_string()
            };

            let border_style = if is_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            };

            let paragraph = Paragraph::new(selected_text)
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(wave.title.clone())
                        .border_style(border_style),
                );

            f.render_widget(paragraph, rows[i]);
        }

        if self.dropdown_open {
            if let Some(wave) = self.waves.get(self.focused_index) {
                let area = rows[self.focused_index];

                let dropdown_height = (wave.items.len() + 1).min(10) as u16 + 2;
                let dropdown_area = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: dropdown_height,
                };

                f.render_widget(Clear, dropdown_area);

                let mut items = vec![ListItem::new("None")];
                items.extend(
                    wave.items
                        .iter()
                        .map(|item| ListItem::new(item.title.clone())),
                );

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .style(Style::default().bg(Color::Black)),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );

                f.render_stateful_widget(list, dropdown_area, &mut self.dropdown_state);
            }
        }

        let instructions = Paragraph::new(
            "Up/Down: Navigate | Enter: Select/Open | Ctrl+Space: Start Wave | s: Toggle Settings",
        )
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
        f.render_widget(instructions, chunks[1]);
    }
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub struct RenderRequest {
    pub width: usize,
    pub height: usize,
    pub time: f32,
    pub amplitude: f32,
    pub bg_rgb: (u8, u8, u8),
    pub palette: [Vector3; 6],
    pub buffer: Vec<((u8, u8, u8), (u8, u8, u8))>,
}

pub struct RenderResult {
    pub width: usize,
    pub height: usize,
    pub data: Vec<((u8, u8, u8), (u8, u8, u8))>,
}

struct LayerConstants {
    intensity: f32,
    edge_offset: f32,
    scale: f32,
    outer_radius_bias: f32,
    highlight_factor: f32,
    offset: f32,
    color1: Vector3,
    color2: Vector3,
}

fn calculate_noise_shape(
    uv: Vector3,
    len: f32,
    constants: &LayerConstants,
    time: f32,
) -> (Vector3, f32) {
    let len_scaled = len * constants.scale;
    if len_scaled > 1.5 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let n0 = simplex_noise(
        uv.x * constants.scale * 0.95 + constants.offset,
        uv.y * constants.scale * 0.95 + constants.offset,
        time * 0.5 + constants.offset,
    ) * 0.5
        + 0.5;
    let r0 = n0;

    let d0 = (len_scaled - r0).abs();

    let edge0 = r0 + constants.edge_offset;
    let v0 = smooth_step(edge0, r0, len_scaled);

    let v1 = constants.intensity / (1.0 + d0 + d0 * 70.0);

    if v0 < 0.05 && v1 < 0.02 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let mix_factor = (uv.y * constants.scale * 2.0).clamp(0.0, 1.0);
    let mut col = constants.color1.lerp(constants.color2, mix_factor);

    col.x = (col.x + v1).clamp(0.0, 1.0);
    col.y = (col.y + v1).clamp(0.0, 1.0);
    col.z = (col.z + v1).clamp(0.0, 1.0);

    (col, v0)
}

fn calculate_blob_layer(
    uv: Vector3,
    len: f32,
    blob_radius_param: f32,
    constants: &LayerConstants,
    time: f32,
) -> (Vector3, f32) {
    let outer_radius = blob_radius_param + constants.outer_radius_bias;

    if len > outer_radius + 0.6 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let (mut noise_col, mut noise_alpha) = calculate_noise_shape(uv, len, constants, time);

    let alpha_falloff = smooth_step(outer_radius + 0.35, 0.22, len);
    noise_alpha = lerp(0.0, noise_alpha, alpha_falloff);

    if noise_alpha < 0.03 {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }

    let highlight = constants.highlight_factor * (1.0 - smooth_step(0.2, outer_radius * 0.8, len));
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
    layer_constants: &[LayerConstants; 3],
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

    let v_center = ((y as f32 * 2.0 + 0.5) / (height as f32 * 2.0)) * 2.0 - 1.0;
    let v_center_scaled = v_center * 1.6;
    let n0 = simplex_noise(u_scaled * 0.95, v_center_scaled * 0.95, time * 0.5) * 0.5 + 0.5;

    let len_top = (u_scaled * u_scaled + v_top_scaled * v_top_scaled).sqrt();
    let len_bot = (u_scaled * u_scaled + v_bot_scaled * v_bot_scaled).sqrt();

    for i in 0..3 {
        let constants = &layer_constants[i];
        let radius = 0.6 - 0.12 * (i as f32);
        let blob_param = lerp(radius, radius + 0.15, n0);

        let (col, alpha) = calculate_blob_layer(
            Vector3::new(u_scaled, v_top_scaled, 0.0),
            len_top,
            blob_param,
            constants,
            time,
        );
        top_color = top_color.lerp(col, alpha);
        top_alpha_acc += alpha;

        let (col, alpha) = calculate_blob_layer(
            Vector3::new(u_scaled, v_bot_scaled, 0.0),
            len_bot,
            blob_param,
            constants,
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

    let mut data = req.buffer;
    if data.len() != width * height {
        data.resize(width * height, ((0, 0, 0), (0, 0, 0)));
    }

    let create_layer_constant = |i: usize| {
        let fi = i as f32;
        let width_param = 0.5 - 0.15 * fi;
        let spark = 1.0 - 0.2 * fi;
        let offset = 0.0 + 1.57 * fi;

        let base_reaction = spark * 0.3;
        let like_reaction = amplitude * 0.12;
        let audio_strength = amplitude;

        let subtle_reaction = (like_reaction * 0.3 + audio_strength * 1.2).min(0.95);
        let outer_radius_bias = width_param * 0.5 + base_reaction * (1.0 + subtle_reaction * 1.5);

        let strength = (like_reaction * 0.7 + audio_strength * 0.45).min(0.65);
        let scale = 1.0 - like_reaction * 0.5;

        let intensity =
            0.05 * (1.0 + 0.35 * (-(time * 1.5 + offset * 0.35).sin() * 0.5)) + 0.08 * strength;
        let edge_offset = 0.05 + (time.sin() * 0.2 + 0.25);
        let highlight_factor = 0.6 * like_reaction;

        LayerConstants {
            intensity,
            edge_offset,
            scale,
            outer_radius_bias,
            highlight_factor,
            offset,
            color1: palette[i],
            color2: palette[i + 3],
        }
    };

    let layer_constants = [
        create_layer_constant(0),
        create_layer_constant(1),
        create_layer_constant(2),
    ];

    let num_threads = rayon::current_num_threads();
    let rows_per_chunk = (height / num_threads).max(1);
    let chunk_size = width * rows_per_chunk;

    data.par_chunks_mut(chunk_size)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_y = chunk_idx * rows_per_chunk;
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
                        &layer_constants,
                    );
                }
            }
        });

    RenderResult {
        width,
        height,
        data,
    }
}
