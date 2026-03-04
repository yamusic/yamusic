use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    #[inline(always)]
    pub fn new(val: f32) -> Self {
        Self(AtomicU32::new(val.to_bits()))
    }

    #[inline(always)]
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    #[inline(always)]
    pub fn set(&self, val: f32) {
        self.0.store(val.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct ParamInfo {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

pub struct EffectParams {
    enabled: AtomicBool,
    values: Vec<AtomicF32>,
    info: Vec<ParamInfo>,
}

unsafe impl Send for EffectParams {}
unsafe impl Sync for EffectParams {}

impl EffectParams {
    pub fn new(info: &[ParamInfo]) -> Self {
        Self {
            enabled: AtomicBool::new(false),
            values: info.iter().map(|p| AtomicF32::new(p.default)).collect(),
            info: info.to_vec(),
        }
    }

    #[inline(always)]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn get(&self, idx: usize) -> f32 {
        self.values.get(idx).map_or(0.0, |v| v.get())
    }

    #[inline(always)]
    pub fn set(&self, idx: usize, val: f32) {
        if let Some(atomic) = self.values.get(idx) {
            let info = &self.info[idx];
            atomic.set(val.clamp(info.min, info.max));
        }
    }

    pub fn param_count(&self) -> usize {
        self.values.len()
    }

    pub fn info(&self) -> &[ParamInfo] {
        &self.info
    }
}

#[derive(Clone)]
pub struct EffectHandle {
    pub id: String,
    pub name: String,
    pub(crate) params: Arc<EffectParams>,
}

impl EffectHandle {
    pub fn is_enabled(&self) -> bool {
        self.params.is_enabled()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.params.set_enabled(enabled);
    }

    pub fn get_param(&self, idx: usize) -> f32 {
        self.params.get(idx)
    }

    pub fn set_param(&self, idx: usize, val: f32) {
        self.params.set(idx, val);
    }

    pub fn param_count(&self) -> usize {
        self.params.param_count()
    }
}
