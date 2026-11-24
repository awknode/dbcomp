//! dBcomp — awk studios. A small everyday compressor for inserts (snare, vocals, bass,
//! buses) with a one-knob saturator.
//!
//!   THRESH, RATIO, ATTACK, RELEASE, GAIN.
//!   ARC (auto release): the release follows the material's crest factor: longer on
//!   sustained sounds (no pumping, clean lows), quicker on punchy ones.
//!   ELECTRO / OPTO: how the release moves with the gain reduction. ELECTRO lets go
//!   quickly near 0 dB (punchy); OPTO lets go quickly from deep GR, then eases in slowly.
//!   WARM / SMOOTH: WARM adds a little harmonic colour that grows with the gain reduction.
//!   SAT (+ KEEP LOW / MID / HIGH): the one-knob saturator after the compressor.
//!
//! in ─► detector (linked, ~3 ms RMS) ─► gain computer (6 dB soft knee) ─► GR ballistics
//!    ─► gain ─► [WARM] ─► GAIN ─► SAT ─► MIX with dry ─► SAFE. Zero latency.

use nice_plug::prelude::*;
use nice_plug_egui::EguiEditorState;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

mod awk_ui;
pub mod dbcore;

mod editor;
mod nice_app;
mod presets;

use dbcore::*;

const CR: usize = 16;
/// 0.1.3: fitted to Waves RComp (awk-bench 2026-11-22): 24 dB knee, threshold reads 1 dB lower,
/// ratio law r^1.25 -> GR within ~1 dB of RComp at 2:1 / 4:1 / 8:1 (was 2-3 dB light everywhere).
const KNEE: f32 = 24.0;
const THR_OFFSET: f32 = 1.0;
const RATIO_EXP: f32 = 1.25;
/// RComp's attack knob measures ~1.8x slower (10 ms -> 18 ms to 63%), its release ~0.48x (120 ms -> 58 ms).
const ATTACK_SCALE: f32 = 1.8;
const RELEASE_SCALE: f32 = 0.32;

pub struct Shared {
    pub meters: awk_ui::Meters,
    pub gr: AtomicU32,
}
impl Default for Shared {
    fn default() -> Self {
        Self { meters: awk_ui::Meters::default(), gr: AtomicU32::new(0) }
    }
}
pub fn get(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}
fn put(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

#[derive(Params)]
pub struct DbCompParams {
    /// window size (nice-plug keeps hosts in charge of it; not persisted)
    pub editor_state: Arc<EguiEditorState>,
    #[id = "thresh"]
    pub thresh: FloatParam,
    #[id = "ratio"]
    pub ratio: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "arc"]
    pub arc: BoolParam,
    #[id = "opto"]
    pub opto: BoolParam,
    #[id = "warm"]
    pub warm: BoolParam,
    #[id = "sat"]
    pub sat: FloatParam,
    #[id = "keep"]
    pub keep: IntParam,
    #[id = "gmix"]
    pub mix: FloatParam,
    #[id = "safe"]
    pub safe: BoolParam,
    #[id = "bypass"]
    pub bypass: BoolParam,
    /// Window size (1.0 = 100%), saved with the project. 0 = not set: open at the default.
    #[persist = "ui-scale"]
    pub ui_scale: Arc<std::sync::RwLock<f32>>,
}

fn ms_param(name: &str, def: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-2.0) })
        .with_smoother(SmoothingStyle::Linear(30.0))
        .with_unit(" ms")
        .with_value_to_string(Arc::new(|v| if v < 10.0 { format!("{v:.1}") } else { format!("{v:.0}") }))
}
pub fn sat_param() -> FloatParam {
    FloatParam::new("Saturation", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(30.0))
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
        .with_unit("%")
}
pub fn keep_param() -> IntParam {
    IntParam::new("Sat Clean", 1, IntRange::Linear { min: 0, max: 2 }).with_value_to_string(Arc::new(|v| format!("CLEAN {}", KEEP_NAMES[v.clamp(0, 2) as usize])))
}

impl Default for DbCompParams {
    fn default() -> Self {
        Self {
            editor_state: EguiEditorState::from_size(nice_plug::editor::dpi::LogicalSize::new((editor::WIDTH) as f64, (editor::HEIGHT) as f64), 1.0),
            thresh: FloatParam::new("Threshold", 0.0, FloatRange::Linear { min: -48.0, max: 0.0 }).with_smoother(SmoothingStyle::Linear(30.0)).with_unit(" dB").with_value_to_string(formatters::v2s_f32_rounded(1)),
            ratio: FloatParam::new("Ratio", 4.0, FloatRange::Skewed { min: 1.0, max: 20.0, factor: FloatRange::skew_factor(-1.5) })
                .with_smoother(SmoothingStyle::Linear(30.0))
                .with_value_to_string(Arc::new(|v| format!("{v:.1}:1"))),
            attack: ms_param("Attack", 10.0, 0.5, 200.0),
            release: ms_param("Release", 120.0, 5.0, 2000.0),
            gain: FloatParam::new("Gain", 0.0, FloatRange::Linear { min: -18.0, max: 18.0 }).with_smoother(SmoothingStyle::Linear(30.0)).with_unit(" dB").with_value_to_string(formatters::v2s_f32_rounded(1)),
            arc: BoolParam::new("ARC", true),
            opto: BoolParam::new("Opto", false),
            warm: BoolParam::new("Warm", true),
            sat: sat_param(),
            keep: keep_param(),
            mix: awk_ui::mix_param(),
            safe: awk_ui::safe_param(),
            bypass: awk_ui::bypass_param(),
            ui_scale: Arc::new(std::sync::RwLock::new(0.0)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct State {
    pub thresh: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub gain_db: f32,
    pub arc: bool,
    pub opto: bool,
    pub warm: bool,
    pub sat: f32,
    pub keep: i32,
    pub mix: f32,
    pub safe: bool,
    pub bypass: bool,
}
impl Default for State {
    fn default() -> Self {
        Self { thresh: 0.0, ratio: 4.0, attack_ms: 10.0, release_ms: 120.0, gain_db: 0.0, arc: true, opto: false, warm: true, sat: 0.0, keep: 1, mix: 1.0, safe: true, bypass: false }
    }
}

/// Release time multiplier: behaviour (from the current GR, dB) x ARC (crest factor).
pub fn release_mult(gr: f32, arc_m: f32, arc: bool, opto: bool) -> f32 {
    let e = (-gr / 4.0).exp();
    let beh = if opto { 0.5 + 1.5 * e } else { 1.5 - e };
    RELEASE_SCALE * beh * if arc { arc_m } else { 1.0 }
}

pub struct Engine {
    sr: f32,
    lvl: Level,
    gr: f32,
    crest: Crest,
    ca: f32,
    cr: f32,
    n: usize,
    warm: Sat,
    sat: Sat,
    mix: f32,
    byp: awk_ui::BypassRamp,
    ceil: awk_ui::Ceiling,
    meter: awk_ui::PeakFollower,
    pub shared: Arc<Shared>,
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        let mut e = Self {
            sr,
            lvl: Level::new(sr, 0.003),
            gr: 0.0,
            crest: Crest::new(sr),
            ca: 0.0,
            cr: 0.0,
            n: 0,
            warm: Sat::new(sr),
            sat: Sat::new(sr),
            mix: -1.0,
            byp: awk_ui::BypassRamp::default(),
            ceil: awk_ui::Ceiling::default(),
            meter: awk_ui::PeakFollower::default(),
            shared: Arc::new(Shared::default()),
        };
        e.byp.set_sr(sr);
        e.ceil.set_sr(sr);
        e.meter.set_sr(sr);
        e
    }
    pub fn with_shared(mut self, s: Arc<Shared>) -> Self {
        self.shared = s;
        self
    }
    pub fn reset(&mut self) {
        let sr = self.sr;
        let s = self.shared.clone();
        // the saturators keep their AUTO GAIN level across a host reset (play/stop)
        let mut warm = std::mem::replace(&mut self.warm, Sat::new(sr));
        warm.reset();
        let mut sat = std::mem::replace(&mut self.sat, Sat::new(sr));
        sat.reset();
        *self = Self::new(sr).with_shared(s);
        self.warm = warm;
        self.sat = sat;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], st: &State) {
        if self.mix < 0.0 {
            self.mix = st.mix;
        }
        let am = 1.0 - (-1.0 / (0.02 * self.sr)).exp();
        let warm_on = st.warm;
        let k = db(st.gain_db);
        let mut peak_gr = 0.0f32;
        for i in 0..l.len() {
            let (x0, x1) = (l[i], r[i]);
            if self.n == 0 {
                self.ca = coef(self.sr, st.attack_ms * ATTACK_SCALE * 0.001);
                self.cr = coef(self.sr, st.release_ms * 0.001 * release_mult(self.gr, self.crest.mult(), st.arc, st.opto));
            }
            self.n = (self.n + 1) % CR;
            self.crest.tick(x0, x1);
            // knee widens as THRESH comes down, so THRESH 0 stays a clean "off" (hard knee at 0 dBFS)
            let knee = KNEE.min(-2.0 * st.thresh).max(0.5);
            let t = gr_soft(self.lvl.tick(x0, x1) + THR_OFFSET, st.thresh, st.ratio.max(1.0).powf(RATIO_EXP), knee);
            let c = if t > self.gr { self.ca } else { self.cr };
            self.gr = t + c * (self.gr - t);
            peak_gr = peak_gr.max(self.gr);
            let gc = db(-self.gr);
            let (mut y0, mut y1) = (x0 * gc, x1 * gc);
            if warm_on {
                // quantised so the saturator's cached coefficients hold between steps
                // 0.1.3: RComp's WARM is very subtle (2nd harmonic ~-105 dB); ours was -49 dB. Now ~20 dB subtler.
                let w = ((self.gr / 18.0).min(1.0) * 0.035 * 512.0).round() / 512.0;
                y0 = self.warm.tick(0, y0, w, 1);
                y1 = self.warm.tick(1, y1, w, 1);
            }
            (y0, y1) = self.sat.tick2(y0 * k, y1 * k, st.sat, st.keep);
            self.mix += am * (st.mix - self.mix);
            let (y0, y1) = (x0 + (y0 - x0) * self.mix, x1 + (y1 - x1) * self.mix);
            let w = self.byp.next(st.bypass);
            let (y0, y1) = (x0 + (y0 - x0) * w, x1 + (y1 - x1) * w);
            let (y0, y1) = self.ceil.tick(y0, y1, st.safe);
            self.meter.feed(x0, x1, y0, y1);
            l[i] = y0;
            r[i] = y1;
        }
        put(&self.shared.gr, peak_gr);
        self.meter.store(&self.shared.meters);
        self.ceil.store(&self.shared.meters);
    }
}

pub struct DbComp {
    params: Arc<DbCompParams>,
    shared: Arc<Shared>,
    eng: Engine,
}
impl Default for DbComp {
    fn default() -> Self {
        let shared = Arc::new(Shared::default());
        Self { params: Arc::new(DbCompParams::default()), shared: shared.clone(), eng: Engine::new(48000.0).with_shared(shared) }
    }
}

impl Plugin for DbComp {
    const NAME: &'static str = "dBcomp";
    const VENDOR: &'static str = "awk studios";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout { main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() }];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type Editor = crate::nice_app::AwkEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.shared.clone())
    }
    fn activate(&mut self, _layout: &AudioIOLayout, cfg: &BufferConfig, _ctx: &mut impl ActivateContext<Self>) -> bool {
        self.eng = Engine::new(cfg.sample_rate).with_shared(self.shared.clone());
        true
    }
    fn reset(&mut self) {
        self.eng.reset();
    }
    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, _ctx: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let p = &self.params;
        let st = State {
            thresh: p.thresh.smoothed.next(),
            ratio: p.ratio.smoothed.next(),
            attack_ms: p.attack.smoothed.next(),
            release_ms: p.release.smoothed.next(),
            gain_db: p.gain.smoothed.next(),
            arc: p.arc.value(),
            opto: p.opto.value(),
            warm: p.warm.value(),
            sat: p.sat.smoothed.next(),
            keep: p.keep.value(),
            mix: p.mix.value(),
            safe: p.safe.value(),
            bypass: p.bypass.value(),
        };
        let sl = buffer.as_slice();
        if sl.len() < 2 {
            return ProcessStatus::Normal;
        }
        let n = sl[0].len().saturating_sub(1) as u32;
        let (a, b) = sl.split_at_mut(1);
        self.eng.process(&mut *a[0], &mut *b[0], &st);
        if n > 0 {
            for f in [&p.thresh, &p.ratio, &p.attack, &p.release, &p.gain, &p.sat] {
                f.smoothed.next_step(n);
            }
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for DbComp {
    const CLAP_ID: &'static str = "com.awknode.dbcomp";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Small everyday compressor with a one-knob saturator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Compressor, ClapFeature::Stereo];
}
impl Vst3Plugin for DbComp {
    const VST3_CLASS_ID: [u8; 16] = *b"AwknodeDbComp001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nice_export_clap!(DbComp);
nice_export_vst3!(DbComp);
