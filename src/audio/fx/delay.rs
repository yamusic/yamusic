#[inline(always)]
#[allow(dead_code)]
pub fn fast_tanh(x: f32) -> f32 {
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let a = x + 0.16489087 * x3 + 0.00985468 * x5;
    a / (1.0 + a * a).sqrt()
}

pub struct DelayLine {
    buffer: Vec<f32>,
    pos: usize,
    mask: usize,
}

impl DelayLine {
    pub fn new(min_size: usize) -> Self {
        let size = min_size.max(4).next_power_of_two();
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            mask: size - 1,
        }
    }

    #[inline(always)]
    pub fn write_and_advance(&mut self, sample: f32) {
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) & self.mask;
    }

    #[inline(always)]
    pub fn read(&self, delay: usize) -> f32 {
        let idx = self.pos.wrapping_sub(1).wrapping_sub(delay) & self.mask;
        unsafe { *self.buffer.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn read_linear(&self, delay: f32) -> f32 {
        let d = delay.max(0.0);
        let di = d as usize;
        let frac = d - di as f32;
        let s0 = self.read(di);
        let s1 = self.read(di + 1);
        s0 + frac * (s1 - s0)
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}

pub struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    size: usize,
    pub feedback: f32,
    damp1: f32,
    damp2: f32,
    filter_state: f32,
}

impl CombFilter {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            size,
            feedback: 0.5,
            damp1: 0.5,
            damp2: 0.5,
            filter_state: 0.0,
        }
    }

    pub fn set_damp(&mut self, damp: f32) {
        self.damp1 = 1.0 - damp;
        self.damp2 = damp;
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = unsafe { *self.buffer.get_unchecked(self.pos) };
        self.filter_state = output * self.damp1 + self.filter_state * self.damp2;
        unsafe {
            *self.buffer.get_unchecked_mut(self.pos) = input + self.filter_state * self.feedback;
        }
        self.pos += 1;
        if self.pos >= self.size {
            self.pos = 0;
        }
        output
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.filter_state = 0.0;
        self.pos = 0;
    }
}

pub struct AllpassFilter {
    buffer: Vec<f32>,
    pos: usize,
    size: usize,
    feedback: f32,
}

impl AllpassFilter {
    pub fn new(size: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            size,
            feedback,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let buffered = unsafe { *self.buffer.get_unchecked(self.pos) };
        let output = buffered - input;
        unsafe {
            *self.buffer.get_unchecked_mut(self.pos) = input + buffered * self.feedback;
        }
        self.pos += 1;
        if self.pos >= self.size {
            self.pos = 0;
        }
        output
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}

pub struct OnePole {
    state: f32,
    g: f32,
}

impl Default for OnePole {
    fn default() -> Self {
        Self::new()
    }
}

impl OnePole {
    pub fn new() -> Self {
        Self { state: 0.0, g: 0.0 }
    }

    pub fn set_damp(&mut self, g: f32) {
        self.g = g.clamp(0.0, 0.999);
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        self.state = input * (1.0 - self.g) + self.state * self.g;
        self.state
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }
}

pub struct DcCut {
    x1: f32,
    y1: f32,
    r: f32,
}

impl DcCut {
    pub fn new(sample_rate: f32) -> Self {
        let r = 1.0 - (2.0 * std::f32::consts::PI * 10.0 / sample_rate);
        Self {
            x1: 0.0,
            y1: 0.0,
            r: r.clamp(0.9, 0.9999),
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let y = input - self.x1 + self.r * self.y1;
        self.x1 = input;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

pub struct TankDelay {
    buffer: Vec<f32>,
    pos: usize,
    size: usize,
}

impl TankDelay {
    pub fn new(size: usize) -> Self {
        let size = size.max(1);
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            size,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let tail = self.buffer[self.pos];
        self.buffer[self.pos] = input;
        self.pos += 1;
        if self.pos >= self.size {
            self.pos = 0;
        }
        tail
    }

    #[inline(always)]
    pub fn output(&self) -> f32 {
        self.buffer[self.pos]
    }

    #[inline(always)]
    pub fn tap(&self, index: usize) -> f32 {
        let idx = (self.pos + index) % self.size;
        self.buffer[idx]
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}

pub struct TankAllpass {
    buffer: Vec<f32>,
    pos: usize,
    size: usize,
    pub feedback: f32,
}

impl TankAllpass {
    pub fn new(size: usize, feedback: f32) -> Self {
        let size = size.max(1);
        Self {
            buffer: vec![0.0; size],
            pos: 0,
            size,
            feedback,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let v = input + self.feedback * delayed;
        let output = delayed - self.feedback * v;
        self.buffer[self.pos] = v;
        self.pos += 1;
        if self.pos >= self.size {
            self.pos = 0;
        }
        output
    }

    #[inline(always)]
    pub fn tap(&self, index: usize) -> f32 {
        let idx = (self.pos + index) % self.size;
        self.buffer[idx]
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

pub struct ModulatedAllpass {
    buffer: Vec<f32>,
    pos: usize,
    buf_size: usize,
    base_delay: usize,
    excursion: f32,
    pub feedback: f32,
}

impl ModulatedAllpass {
    pub fn new(delay: usize, excursion: usize, feedback: f32) -> Self {
        let buf_size = delay + 2 * excursion + 4;
        Self {
            buffer: vec![0.0; buf_size],
            pos: 0,
            buf_size,
            base_delay: delay,
            excursion: excursion as f32,
            feedback,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32, modulation: f32) -> f32 {
        let delay_f = self.base_delay as f32 + modulation * self.excursion;
        let delay_f = delay_f.max(1.0);

        let d_floor = delay_f as usize;
        let frac = delay_f - d_floor as f32;

        let idx0 = (self.pos + self.buf_size - d_floor) % self.buf_size;
        let idx1 = if idx0 == 0 {
            self.buf_size - 1
        } else {
            idx0 - 1
        };
        let delayed = self.buffer[idx0] + frac * (self.buffer[idx1] - self.buffer[idx0]);

        let v = input + self.feedback * delayed;
        let output = delayed - self.feedback * v;

        self.buffer[self.pos] = v;
        self.pos = (self.pos + 1) % self.buf_size;

        output
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}
