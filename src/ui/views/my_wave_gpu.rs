use crate::ui::views::my_wave::{RenderRequest, RenderResult};
use pollster::block_on;
use std::borrow::Cow;

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
                .await)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("MyWave Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MyWave Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER_SOURCE)),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MyWave Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MyWave Pipeline Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MyWave Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: Default::default(),
        });

        let capacity = 1024 * 1024 * 8;
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let field_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Storage Buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
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
                label: Some("Storage Buffer"),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Buffer"),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        let mut palette = [[0.0; 4]; 6];
        let base_colors = [
            [0.1, 0.6, 0.9, 1.0],
            [0.9, 0.1, 0.3, 1.0],
            [0.1, 0.9, 0.4, 1.0],
            [0.9, 0.8, 0.1, 1.0],
            [0.6, 0.1, 0.9, 1.0],
            [0.1, 0.9, 0.9, 1.0],
        ];

        for i in 0..6 {
            palette[i] = base_colors[i];
        }

        let orbit_vectors = [
            [0.5, 0.5, 0.2, 0.0],
            [0.2, 0.8, -0.3, 0.0],
            [0.8, 0.2, 0.4, 0.0],
        ];

        let level = req.amplitude;
        let spectral = [level, level * 0.8, level * 0.6, 0.0];
        let reactive = [level * 1.2, level, level * 0.8, 0.0];

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
            bands: palette,
            orbits: orbit_vectors,
            spectrum: spectral,
            response: reactive,
        };

        self.queue
            .write_buffer(&self.params_buf, 0, bytemuck::cast_slice(&[params]));

        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
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
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            let block = 16;
            let gx = (w as u32 + block - 1) / block;
            let gy = (h as u32 + block - 1) / block;
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

        self.device.poll(wgpu::Maintain::Wait);

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

            RenderResult {
                width: w,
                height: h,
                data: req.buffer,
            }
        } else {
            RenderResult {
                width: w,
                height: h,
                data: vec![((0, 0, 0), (0, 0, 0)); w * h],
            }
        }
    }
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

fn taylor_inv_sqrt(r: vec4<f32>) -> vec4<f32> {
    return 1.79284291400159 - 0.85373472095314 * r;
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

    let norm = taylor_inv_sqrt(vec4<f32>(dot(q0, q0), dot(q1, q1), dot(q2, q2), dot(q3, q3)));
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

    for (var i = 0; i <= 4; i++) {
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

fn shape_noise_lobe(uv: vec2<f32>, c1: vec3<f32>, c2: vec3<f32>, strength: f32, ofs: f32) -> vec4<f32> {
    let len_uv = length(uv);
    let n_val = simplex3(vec3<f32>(uv * 1.2 + ofs, p.phase * 0.5 + ofs)) * 0.5 + 0.5;
    let r0 = n_val;
    let radial = distance(uv, (r0 / len_uv) * uv);

    let edge = r0 + 0.1 + (sin(p.phase + ofs) + 1.0);
    let mask = smoothstep(edge, r0, len_uv);

    let light_power = 0.15 * (1.0 + 1.5 * (-sin(p.phase * 2.0 + ofs * 0.5) * 0.5)) + 0.3 * strength;
    let glow = falloff(light_power, 10.0, radial);

    var col = c1 + (c2 - c1) * clamp(uv.y * 2.0, 0.0, 1.0);
    col = col + glow;
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(col, mask);
}

fn compose_ring(
    uv: vec2<f32>,
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

    var blob = shape_noise_lobe(uv * (1.0 - like_val * 0.5) + noise_pos, c1, c2, strength, phase_ofs);

    let alpha_scale = smoothstep(outer, 0.5, len_uv);
    blob.w = blob.w * alpha_scale;

    let halo = 0.6 * like_val * (1.0 - smoothstep(0.2, outer * 0.8, len_uv));
    let boosted = blob.xyz + vec3<f32>(halo, halo, halo);
    return vec4<f32>(boosted, blob.w);
}

fn shade_cell(ix: u32, iy: u32, lower_half: bool) -> vec3<f32> {
    let w = p.viewport.x;
    let h = p.viewport.y;

    var y_norm: f32;
    if (lower_half) {
        y_norm = (f32(iy) * 2.0 + 1.0) / (h * 2.0);
    } else {
        y_norm = (f32(iy) * 2.0) / (h * 2.0);
    }
    let x_norm = f32(ix) / w;

    var uv = vec2<f32>(x_norm, y_norm) * 2.0 - 1.0;

    let min_dim = min(w, h * 2.0);
    let sx = w / min_dim / p.zoom;
    let sy = (h * 2.0) / min_dim / p.zoom;
    uv.x = uv.x * sx;
    uv.y = -uv.y * sy;

    let ruv = uv * 2.0;
    let pa = atan2(ruv.y, ruv.x);
    let idx = (pa / 3.1415) * 0.5;

    let ruv1 = spin2d(uv * 2.0, 3.1415);
    let pa1 = atan2(ruv1.y, ruv1.x);
    let idx1 = (pa1 / 3.1415) * 0.5;
    let idx21 = (pa1 / 3.1415 + 1.0) * 0.5 * 3.1415;

    var spark = tri_field3d(vec3<f32>(idx, 0.0, 0.0), 0.1);
    let spark2 = tri_field3d(vec3<f32>(idx1, 0.0, idx1), 0.1);
    let mix_amount = smoothstep(0.9, 1.0, sin(idx21));
    spark = spark * (1.0 - mix_amount) + spark2 * mix_amount;
    spark = spark * 0.2 + pow(spark, 10.0);
    spark = smoothstep(0.0, spark, 0.3) * spark;

    var color = p.backdrop.xyz;
    let n0 = simplex3(vec3<f32>(uv * 1.2, p.phase * 0.5));

    for (var i: i32 = 0; i < 3; i++) {
        let idx_f = f32(i);
        let radius = BASE_RADIUS - STEP_RADIUS * idx_f;

        let col1 = p.bands[i].xyz;
        let col2 = p.bands[i + 3].xyz;
        let rot = p.orbits[i];

        var band_val: f32;
        var react_val: f32;
        if (i == 0) {
            band_val = p.spectrum.x;
            react_val = p.response.x;
        } else if (i == 1) {
            band_val = p.spectrum.y;
            react_val = p.response.y;
        } else {
            band_val = p.spectrum.z;
            react_val = p.response.z;
        }

        let mixed_r = radius + (radius + 0.3 - radius) * n0;
        let w_ring = BASE_WIDTH - STEP_WIDTH * idx_f;
        let spark_base = (BASE_SPARK - STEP_SPARK * idx_f) * spark;
        let off = BASE_OFFSET + STEP_OFFSET * idx_f;
        let noise_pos = spin2d(rot.xy, p.phase * rot.z);

        let blob = compose_ring(
            uv,
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

    let top_color = shade_cell(x, y, false);
    let bot_color = shade_cell(x, y, true);

    let idx = y * u32(p.viewport.x) + x;
    out_cells[idx * 2] = pack_rgb(top_color);
    out_cells[idx * 2 + 1] = pack_rgb(bot_color);
}
"#;
