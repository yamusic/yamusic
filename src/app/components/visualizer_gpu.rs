use pollster::block_on;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use yandex_music::model::track::Track;

use crate::framework::{
    component::{Component, ComponentCore},
    id::ComponentId,
    signals::Signal,
    theme::ThemeStyles,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParams {
    viewport: [f32; 2],
    phase: f32,
    zoom: f32,
    backdrop: [f32; 4],
    bands: [[f32; 4]; 6],
    orbits: [[f32; 4]; 3],
    spectrum: [f32; 4],
    response: [f32; 4],
}

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    field_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    capacity: usize,
}

impl GpuRenderer {
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .or(instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: false,
                    compatible_surface: None,
                })
                .await)
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Visualizer Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visualizer Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADER_SOURCE)),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visualizer Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let capacity = 1024 * 1024 * 8;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visualizer Pipeline Layout"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Visualizer Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visualizer Uniform Buffer"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let field_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visualizer Storage Buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visualizer Staging Buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Some(Self {
            device,
            queue,
            pipeline,
            layout,
            params_buf,
            field_buf,
            readback_buf,
            bind_group: None,
            capacity: capacity as usize,
        })
    }

    pub fn new_blocking() -> Option<Self> {
        block_on(Self::new())
    }

    pub fn render(&mut self, mut req: RenderRequest) -> RenderResult {
        let w = req.width;
        let h = req.height;
        let bytes_needed = w * h * 8;

        if bytes_needed > self.capacity {
            self.capacity = bytes_needed.next_power_of_two();
            self.field_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Visualizer Storage Buffer"),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Visualizer Staging Buffer"),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_group = None;
        }

        let orbit_vectors = [
            [0.5, 0.5, 0.2, 0.0],
            [0.2, 0.8, -0.3, 0.0],
            [0.8, 0.2, 0.4, 0.0],
        ];

        let level = req.amplitude;
        let spectral = [level, level * 0.8, level * 0.6, 0.0];
        let reactive = [level * 1.2, level, level * 0.8, req.glow];

        let params = GpuParams {
            viewport: [w as f32, h as f32],
            phase: req.time,
            zoom: 0.8,
            backdrop: [
                req.bg_rgb.0 as f32 / 255.0,
                req.bg_rgb.1 as f32 / 255.0,
                req.bg_rgb.2 as f32 / 255.0,
                1.0,
            ],
            bands: req.palette,
            orbits: orbit_vectors,
            spectrum: spectral,
            response: reactive,
        };

        self.queue
            .write_buffer(&self.params_buf, 0, bytemuck::cast_slice(&[params]));
        if self.bind_group.is_none() {
            self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Visualizer Bind Group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.field_buf.as_entire_binding(),
                    },
                ],
            }));
        }
        let bindings = self.bind_group.as_ref().unwrap();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Visualizer Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Visualizer Compute Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bindings, &[]);
            let block = 16;
            let gx = (w as u32).div_ceil(block);
            let gy = (h as u32).div_ceil(block);
            pass.dispatch_workgroups(gx, gy, 1);
        }

        encoder.copy_buffer_to_buffer(
            &self.field_buf,
            0,
            &self.readback_buf,
            0,
            bytes_needed as u64,
        );

        self.queue.submit(Some(encoder.finish()));

        let slice = self.readback_buf.slice(0..bytes_needed as u64);
        let (tx, rx) = flume::bounded(1);
        slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());

        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        if rx.recv().unwrap().is_ok() {
            let mapped = slice.get_mapped_range();
            let pixels: &[u32] = bytemuck::cast_slice(&mapped);

            if req.buffer.len() != w * h {
                req.buffer.resize(w * h, ((0, 0, 0), (0, 0, 0)));
            }

            for i in 0..(w * h) {
                let hi = pixels[i * 2];
                let lo = pixels[i * 2 + 1];

                let top = (
                    ((hi >> 16) & 0xFF) as u8,
                    ((hi >> 8) & 0xFF) as u8,
                    (hi & 0xFF) as u8,
                );
                let bot = (
                    ((lo >> 16) & 0xFF) as u8,
                    ((lo >> 8) & 0xFF) as u8,
                    (lo & 0xFF) as u8,
                );
                req.buffer[i] = (top, bot);
            }

            drop(mapped);
            self.readback_buf.unmap();

            RenderResult { data: req.buffer }
        } else {
            RenderResult {
                data: vec![((0, 0, 0), (0, 0, 0)); w * h],
            }
        }
    }
}

pub struct RenderRequest {
    pub width: usize,
    pub height: usize,
    pub time: f32,
    pub amplitude: f32,
    pub glow: f32,
    pub bg_rgb: (u8, u8, u8),
    pub palette: [[f32; 4]; 6],
    pub buffer: Vec<((u8, u8, u8), (u8, u8, u8))>,
}

pub struct RenderResult {
    pub data: Vec<((u8, u8, u8), (u8, u8, u8))>,
}

#[derive(Clone)]
struct SharedVisualizerParams {
    speed: f32,
    amplitude: f32,
    glow: f32,
    bg_rgb: (u8, u8, u8),
    palette: [[f32; 4]; 6],
    width: usize,
    height: usize,
}

struct VisualizerState {
    smoothed_amplitude: f32,
    smoothed_speed: f32,
    last_tick: Instant,
    front_buffer: Option<RenderResult>,
    current_palette: [[f32; 4]; 6],
    current_bg: [f32; 3],
    like_glow_target: f32,
    like_glow_smoothed: f32,
    shared_params: Arc<Mutex<SharedVisualizerParams>>,
    latest_frame: Arc<Mutex<Option<RenderResult>>>,
}

pub struct Visualizer {
    core: ComponentCore,
    amplitude: Signal<f32>,
    is_playing: Signal<bool>,
    current_track: Signal<Option<Track>>,
    theme: Signal<ThemeStyles>,
    state: Mutex<VisualizerState>,
}

impl Visualizer {
    pub fn new(
        amplitude: Signal<f32>,
        is_playing: Signal<bool>,
        current_track: Signal<Option<Track>>,
        theme: Signal<ThemeStyles>,
    ) -> Self {
        let shared_params = Arc::new(Mutex::new(SharedVisualizerParams {
            speed: 0.8,
            amplitude: 0.0,
            glow: 0.0,
            bg_rgb: (13, 13, 13),
            palette: [[0.0; 4]; 6],
            width: 0,
            height: 0,
        }));
        let latest_frame: Arc<Mutex<Option<RenderResult>>> = Arc::new(Mutex::new(None));

        let weak_params = Arc::downgrade(&shared_params);
        let weak_frame = Arc::downgrade(&latest_frame);

        thread::Builder::new()
            .name("visualizer-renderer".to_string())
            .spawn(move || {
                let mut gpu_renderer = GpuRenderer::new_blocking();
                let mut phase: f32 = 0.0;
                let mut last_render = Instant::now();
                let mut buffer: Vec<((u8, u8, u8), (u8, u8, u8))> = Vec::new();
                let target_frame_time = Duration::from_millis(13);

                loop {
                    let params_arc = match weak_params.upgrade() {
                        Some(arc) => arc,
                        None => break,
                    };
                    let frame_arc = match weak_frame.upgrade() {
                        Some(arc) => arc,
                        None => break,
                    };

                    let params = params_arc.lock().unwrap().clone();
                    drop(params_arc);

                    if params.width == 0 || params.height == 0 {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    let now = Instant::now();
                    let dt = now.duration_since(last_render).as_secs_f32().min(0.1);
                    last_render = now;
                    phase += dt * params.speed;

                    let w = params.width;
                    let h = params.height;

                    let req = RenderRequest {
                        width: w,
                        height: h,
                        time: phase,
                        amplitude: params.amplitude,
                        glow: params.glow,
                        bg_rgb: params.bg_rgb,
                        palette: params.palette,
                        buffer: std::mem::take(&mut buffer),
                    };

                    let result = if let Some(renderer) = &mut gpu_renderer {
                        renderer.render(req)
                    } else {
                        thread::sleep(Duration::from_millis(50));
                        RenderResult {
                            data: vec![((0, 0, 0), (0, 0, 0)); w * h],
                        }
                    };

                    let old = frame_arc.lock().unwrap().replace(result);
                    drop(frame_arc);

                    if let Some(old_frame) = old {
                        buffer = old_frame.data;
                    }

                    let elapsed = last_render.elapsed();
                    if elapsed < target_frame_time {
                        thread::sleep(target_frame_time - elapsed);
                    }
                }
            })
            .expect("Failed to spawn visualizer renderer thread");

        Self {
            core: ComponentCore::new(ComponentId::new("visualizer")),
            amplitude,
            is_playing,
            current_track,
            theme,
            state: Mutex::new(VisualizerState {
                smoothed_amplitude: 0.0,
                smoothed_speed: 0.8,
                last_tick: Instant::now(),
                front_buffer: None,
                current_palette: [[0.0; 4]; 6],
                current_bg: [0.0; 3],
                like_glow_target: 0.0,
                like_glow_smoothed: 0.0,
                shared_params,
                latest_frame,
            }),
        }
    }

    fn theme_palette(&self) -> ([[f32; 4]; 6], (u8, u8, u8)) {
        let styles = self.theme.get();
        let bg_rgb = color_to_rgb(styles.text.bg.unwrap_or_default(), (13, 13, 13));

        if let Some(track) = self.current_track.get()
            && let Some(dc) = track.derived_colors
        {
            let accent_rgb = parse_hex_color(&dc.accent).unwrap_or_else(|| {
                color_to_rgb(styles.accent.fg.unwrap_or_default(), (247, 212, 75))
            });
            let wave_rgb =
                parse_hex_color(&dc.wave_text).unwrap_or_else(|| rotate_hue(accent_rgb, 40.0));
            let mini_rgb =
                parse_hex_color(&dc.mini_player).unwrap_or_else(|| rotate_hue(accent_rgb, -40.0));
            let avg_rgb =
                parse_hex_color(&dc.average).unwrap_or_else(|| rotate_hue(accent_rgb, 180.0));

            let palette = [
                rgb_to_vec4(accent_rgb),
                rgb_to_vec4(wave_rgb),
                rgb_to_vec4(mini_rgb),
                rgb_to_vec4(avg_rgb),
                rgb_to_vec4(rotate_hue(wave_rgb, 30.0)),
                rgb_to_vec4(rotate_hue(mini_rgb, -25.0)),
            ];
            return (palette, bg_rgb);
        }

        let base_rgb = color_to_rgb(styles.accent.fg.unwrap_or_default(), (247, 212, 75));
        let palette = [
            rgb_to_vec4(base_rgb),
            rgb_to_vec4(rotate_hue(base_rgb, 40.0)),
            rgb_to_vec4(rotate_hue(base_rgb, -40.0)),
            rgb_to_vec4(shift_rgb(rotate_hue(base_rgb, 180.0), 0.1)),
            rgb_to_vec4(rotate_hue(base_rgb, 20.0)),
            rgb_to_vec4(rotate_hue(base_rgb, -20.0)),
        ];
        (palette, bg_rgb)
    }

    pub fn trigger_like_glow(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.like_glow_target = 1.0;
        }
    }
}

fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    if s.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

impl Component for Visualizer {
    type Message = ();

    fn core(&self) -> &ComponentCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut ComponentCore {
        &mut self.core
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(self, area);
    }
}

impl Widget for &Visualizer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let mut state = self.state.lock().unwrap();

        let now = Instant::now();
        let dt = now.duration_since(state.last_tick).as_secs_f32().min(0.1);
        state.last_tick = now;

        let width = area.width as usize;
        let height = area.height as usize;
        if width == 0 || height == 0 {
            return;
        }

        let signal_amp = self.amplitude.get();
        let raw_amplitude = signal_amp.clamp(0.0, 1.0);

        if raw_amplitude > state.smoothed_amplitude {
            state.smoothed_amplitude = state.smoothed_amplitude * 0.85 + raw_amplitude * 0.15;
        } else {
            state.smoothed_amplitude = state.smoothed_amplitude * 0.95 + raw_amplitude * 0.05;
        }

        let amplitude = state.smoothed_amplitude;

        if state.like_glow_target > 0.0 {
            state.like_glow_target = (state.like_glow_target - dt * 0.4).max(0.0);
        }
        let glow_lerp = if state.like_glow_target > state.like_glow_smoothed {
            dt * 5.0
        } else {
            dt * 2.0
        };
        state.like_glow_smoothed +=
            (state.like_glow_target - state.like_glow_smoothed) * glow_lerp.min(1.0);
        let effective_amplitude = (amplitude + state.like_glow_smoothed * 0.8).min(1.0);

        let target_speed = if self.is_playing.get() {
            0.8 + amplitude * 1.5
        } else {
            0.8
        };
        state.smoothed_speed += (target_speed - state.smoothed_speed) * (dt * 3.0).min(1.0);

        let (target_palette, target_bg_rgb) = self.theme_palette();
        let target_bg = [
            target_bg_rgb.0 as f32 / 255.0,
            target_bg_rgb.1 as f32 / 255.0,
            target_bg_rgb.2 as f32 / 255.0,
        ];
        let lerp_t = (dt * 6.0).min(1.0);
        for i in 0..6 {
            for j in 0..4 {
                state.current_palette[i][j] +=
                    (target_palette[i][j] - state.current_palette[i][j]) * lerp_t;
            }
        }
        for i in 0..3 {
            state.current_bg[i] += (target_bg[i] - state.current_bg[i]) * lerp_t;
        }
        let bg_rgb = (
            (state.current_bg[0] * 255.0) as u8,
            (state.current_bg[1] * 255.0) as u8,
            (state.current_bg[2] * 255.0) as u8,
        );

        {
            let mut p = state.shared_params.lock().unwrap();
            p.speed = state.smoothed_speed;
            p.amplitude = effective_amplitude;
            p.glow = state.like_glow_smoothed;
            p.bg_rgb = bg_rgb;
            p.palette = state.current_palette;
            p.width = width;
            p.height = height;
        }

        let new_frame = state.latest_frame.lock().unwrap().take();
        if let Some(frame) = new_frame {
            state.front_buffer = Some(frame);
        }

        if let Some(frame) = &state.front_buffer
            && frame.data.len() == width * height
        {
            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    let ((r_top, g_top, b_top), (r_bot, g_bot, b_bot)) = frame.data[idx];
                    if let Some(cell) =
                        buf.cell_mut((area.left() + x as u16, area.top() + y as u16))
                    {
                        cell.set_char('▀')
                            .set_fg(Color::Rgb(r_top, g_top, b_top))
                            .set_bg(Color::Rgb(r_bot, g_bot, b_bot))
                            .set_style(Style::default());
                    }
                }
            }
        }
    }
}

fn color_to_rgb(color: Color, fallback: (u8, u8, u8)) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => fallback,
    }
}

fn shift_rgb(rgb: (u8, u8, u8), shift: f32) -> (u8, u8, u8) {
    let (r, g, b) = rgb;
    let adjust = |c: u8| {
        let cf = c as f32 / 255.0;
        let shifted = (cf + shift).clamp(0.0, 1.0);
        (shifted * 255.0) as u8
    };
    (adjust(r), adjust(g), adjust(b))
}

fn rgb_to_vec4(rgb: (u8, u8, u8)) -> [f32; 4] {
    [
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
        1.0,
    ]
}

fn rotate_hue(rgb: (u8, u8, u8), degrees: f32) -> (u8, u8, u8) {
    let r = rgb.0 as f32 / 255.0;
    let g = rgb.1 as f32 / 255.0;
    let b = rgb.2 as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let mut h;
    let s;
    let l = (max + min) / 2.0;

    if max == min {
        h = 0.0;
        s = 0.0;
    } else {
        let d = max - min;
        s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        if max == r {
            h = (g - b) / d + (if g < b { 6.0 } else { 0.0 });
        } else if max == g {
            h = (b - r) / d + 2.0;
        } else {
            h = (r - g) / d + 4.0;
        }
        h /= 6.0;
    }

    h = (h + degrees / 360.0).rem_euclid(1.0);

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hue_to_rgb = |p: f32, q: f32, mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };

    (
        (hue_to_rgb(p, q, h + 1.0 / 3.0) * 255.0) as u8,
        (hue_to_rgb(p, q, h) * 255.0) as u8,
        (hue_to_rgb(p, q, h - 1.0 / 3.0) * 255.0) as u8,
    )
}

const SHADER_SOURCE: &str = r#"
struct Params {
    viewport: vec2<f32>,
    phase: f32,
    zoom: f32,
    backdrop: vec4<f32>,
    bands: array<vec4<f32>, 6>,
    orbits: array<vec4<f32>, 3>,
    spectrum: vec4<f32>,
    response: vec4<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read_write> out_cells: array<u32>;

const BASE_WIDTH: f32 = 0.8;
const STEP_WIDTH: f32 = 0.2;
const BASE_SPARK: f32 = 1.0;
const STEP_SPARK: f32 = 0.3;
const BASE_RADIUS: f32 = 0.95;
const STEP_RADIUS: f32 = 0.15;
const BASE_OFFSET: f32 = 0.0;
const STEP_OFFSET: f32 = 1.57;

fn mod_v3(v: vec3<f32>, d: f32) -> vec3<f32> {
    return v - d * floor(v / d);
}

fn mod_v4(v: vec4<f32>, d: f32) -> vec4<f32> {
    return v - d * floor(v / d);
}

fn perm(v: vec4<f32>) -> vec4<f32> {
    return mod_v4(((v * 34.0) + 1.0) * v, 289.0);
}

fn simplex3(pos: vec3<f32>) -> f32 {
    let C = vec2<f32>(0.1666667, 0.3333333);
    let D = vec4<f32>(0.0, 0.5, 1.0, 2.0);

    var i = floor(pos + dot(pos, C.yyy));
    let x0 = pos - i + dot(i, C.xxx);

    let g = step(x0.yzx, x0.xyz);
    let l = 1.0 - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);

    let x1 = x0 - i1 + C.xxx;
    let x2 = x0 - i2 + C.yyy;
    let x3 = x0 - D.yyy;

    i = mod_v3(i, 289.0);
    let p_hash = perm(perm(perm(i.z + vec4<f32>(0.0, i1.z, i2.z, 1.0)) + i.y + vec4<f32>(0.0, i1.y, i2.y, 1.0)) + i.x + vec4<f32>(0.0, i1.x, i2.x, 1.0));

    let n_ = 0.142857142857;
    let ns = n_ * D.wyz - D.xzx;

    let j = p_hash - 49.0 * floor(p_hash * ns.z * ns.z);

    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);

    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(x) - abs(y);

    let b0 = vec4<f32>(x.xy, y.xy);
    let b1 = vec4<f32>(x.zw, y.zw);

    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4<f32>(0.0));

    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;

    var q0 = vec3<f32>(a0.xy, h.x);
    var q1 = vec3<f32>(a0.zw, h.y);
    var q2 = vec3<f32>(a1.xy, h.z);
    var q3 = vec3<f32>(a1.zw, h.w);

    let norm = inverseSqrt(vec4<f32>(dot(q0, q0), dot(q1, q1), dot(q2, q2), dot(q3, q3)));
    q0 = q0 * norm.x;
    q1 = q1 * norm.y;
    q2 = q2 * norm.z;
    q3 = q3 * norm.w;

    var m = max(0.6 - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4<f32>(0.0));
    m = m * m;
    return 42.0 * dot(m * m, vec4<f32>(dot(q0, x0), dot(q1, x1), dot(q2, x2), dot(q3, x3)));
}

fn tri_wave(x: f32) -> f32 {
    return abs(fract(x) - 0.5);
}

fn tri_vec(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        tri_wave(p.z + tri_wave(p.y * 20.0)),
        tri_wave(p.z + tri_wave(p.x * 1.0)),
        tri_wave(p.y + tri_wave(p.x * 1.0)),
    );
}

fn tri_field3d(seed: vec3<f32>, spd: f32) -> f32 {
    var z = 0.4;
    var acc = 0.1;
    var p0 = seed;
    var base = seed;

    for (var i = 0; i < 4; i++) {
        let g = tri_vec(base * 0.01);
        p0 = p0 + (g + p.phase * 0.1 * spd);
        base = base * 4.0;
        z = z * 0.9;
        p0 = p0 * 1.6;
        acc = acc + tri_wave(p0.z + tri_wave(0.6 * p0.x + 0.1 * tri_wave(p0.y))) / z;
    }
    let s = acc + sin(acc + sin(z) * 2.8) * 2.2;
    return smoothstep(0.0, 8.0, s);
}

fn spin2d(v: vec2<f32>, ang: f32) -> vec2<f32> {
    let s = sin(ang);
    let c = cos(ang);
    return vec2<f32>(v.x * c - v.y * s, v.x * s + v.y * c);
}

fn falloff(intensity: f32, atten: f32, dist: f32) -> f32 {
    return intensity / (1.0 + dist + dist * atten);
}

fn shape_noise_lobe(uv: vec2<f32>, n_val: f32, c1: vec3<f32>, c2: vec3<f32>, strength: f32, ofs: f32) -> vec4<f32> {
    let len_uv = length(uv);
    let r0 = n_val;
    let safe_len = max(len_uv, 1e-4);
    let on_ring = (r0 / safe_len) * uv;
    let radial = distance(uv, on_ring);

    let edge = r0 + 0.1 + (sin(p.phase + ofs) + 1.0);
    let mask = smoothstep(edge, r0, len_uv);

    let light_power = 0.07 * (1.0 + 1.5 * (-sin(p.phase * 2.0 + ofs * 0.5) * 0.5)) + 0.15 * strength;
    let glow = falloff(light_power, 10.0, radial);

    var col = c1 + (c2 - c1) * clamp(uv.y * 2.0, 0.0, 1.0);
    col = col + col * glow;
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(col, mask);
}

fn compose_ring(
    uv: vec2<f32>,
    n_val: f32,
    base_r: f32,
    c1: vec3<f32>,
    c2: vec3<f32>,
    width: f32,
    spark: f32,
    like_val: f32,
    audio_val: f32,
    phase_ofs: f32,
    noise_pos: vec2<f32>,
) -> vec4<f32> {
    let len_uv = length(uv);
    let gain = max(like_val, audio_val * 0.6);
    let outer = base_r + width * 0.5 + spark * (1.0 + gain * 50.0 * spark);
    let strength = max(like_val, audio_val);

    var blob = shape_noise_lobe(
        uv * (1.0 - like_val * 0.5) + noise_pos,
        n_val,
        c1, c2, strength, phase_ofs
    );

    let alpha_scale = smoothstep(outer, 0.5, len_uv);
    blob.w = blob.w * alpha_scale;

    let halo = 0.25 * like_val * (1.0 - smoothstep(0.2, outer * 0.8, len_uv));
    let boosted = blob.xyz + c1 * halo;
    return vec4<f32>(boosted, blob.w);
}

fn get_uv(ix: u32, iy: u32, y_offset: f32) -> vec2<f32> {
    let w = p.viewport.x;
    let h = p.viewport.y;
    let y_norm = (f32(iy) * 2.0 + y_offset) / (h * 2.0);
    let x_norm = (f32(ix) + 0.5) / w;
    var uv = vec2<f32>(x_norm, y_norm) * 2.0 - 1.0;
    let min_dim = min(w, h * 2.0);
    let sx = w / min_dim / p.zoom;
    let sy = (h * 2.0) / min_dim / p.zoom;
    uv.x = uv.x * sx;
    uv.y = -uv.y * sy;
    return uv;
}

fn calculate_spark(uv: vec2<f32>) -> f32 {
    let len = length(uv);
    let dir = select(vec2<f32>(1.0, 0.0), uv / len, len > 0.0001);
    let seed = vec3<f32>(dir.x, dir.y, 0.0) * 0.16;

    var spark = tri_field3d(seed, 0.1);
    let s2 = spark * spark;
    let s10 = (s2 * s2) * (s2 * s2) * s2;
    spark = spark * 0.2 + s10;

    return smoothstep(0.0, max(spark, 1e-5), 0.3) * spark;
}

fn get_pixel_color(uv: vec2<f32>) -> vec3<f32> {
    let spark = calculate_spark(uv);
    let n0 = simplex3(vec3<f32>(uv * 1.2, p.phase * 0.5));

    var color = p.backdrop.xyz;

    for (var i: i32 = 0; i < 3; i++) {
        let idx_f = f32(i);
        let rot = p.orbits[i];
        let off = BASE_OFFSET + STEP_OFFSET * idx_f;
        let react_val = p.response[i];

        let noise_pos = spin2d(rot.xy, p.phase * rot.z);
        let transformed_uv = uv * (1.0 - react_val * 0.5) + noise_pos;
        let band_noise = simplex3(vec3<f32>(transformed_uv * 1.2 + off, p.phase * 0.5 + off)) * 0.5 + 0.5;

        let radius = BASE_RADIUS - STEP_RADIUS * idx_f;
        let col1 = p.bands[i].xyz;
        let col2 = p.bands[i + 3].xyz;
        let band_val = p.spectrum[i];

        let mixed_r = radius + 0.3 * n0;
        let w_ring = BASE_WIDTH - STEP_WIDTH * idx_f;
        let spark_base = (BASE_SPARK - STEP_SPARK * idx_f) * spark;

        let blob = compose_ring(
            uv,
            band_noise,
            mixed_r,
            col1,
            col2,
            w_ring,
            spark_base,
            react_val,
            band_val,
            off,
            noise_pos,
        );

        color = color + (blob.xyz - color) * blob.w;
    }

    let like_glow = p.response.w;
    if like_glow > 0.0 {
        let brightness = like_glow * 1.1;
        let boost_color = mix(p.bands[0].xyz, vec3<f32>(1.0, 1.0, 1.0), 0.4);
        color = color + boost_color * brightness * (1.0 - length(uv) * 0.6);
    }

    return color;
}

fn pack_rgb(c: vec3<f32>) -> u32 {
    let r = u32(clamp(c.r, 0.0, 1.0) * 255.0);
    let g = u32(clamp(c.g, 0.0, 1.0) * 255.0);
    let b = u32(clamp(c.b, 0.0, 1.0) * 255.0);
    return (r << 16u) | (g << 8u) | b;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;

    if (x >= u32(p.viewport.x) || y >= u32(p.viewport.y)) {
        return;
    }

    let uv_top = get_uv(x, y, 0.5);
    let top_color = get_pixel_color(uv_top);

    let uv_bot = get_uv(x, y, 1.5);
    let bot_color = get_pixel_color(uv_bot);

    let idx = y * u32(p.viewport.x) + x;
    out_cells[idx * 2] = pack_rgb(top_color);
    out_cells[idx * 2 + 1] = pack_rgb(bot_color);
}
"#;
