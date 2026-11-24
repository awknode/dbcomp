//! dB series core (shared by dBcomp and dBvox): the one-knob saturator and helpers.
//!
//! SAT: one knob. 0 = bit-exact bypass. Turning it up drives an asymmetric tanh
//! (even + odd harmonics, anti-aliased with first-order ADAA, so no oversampling and
//! no latency), with AUTO GAIN so the knob adds colour, not volume.
//! KEEP: LOW (lows pass clean, the rest is saturated), MID (everything), HIGH (highs pass
//! clean, the body is saturated). Complementary one-pole split, so the clean band and
//! the saturated band always sum back flat.

#[inline]
pub fn db(x: f32) -> f32 {
    10f32.powf(x * 0.05)
}
#[inline]
pub fn to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

/// ln(cosh(v)), overflow-safe: the antiderivative of tanh.
#[inline]
fn lncosh(v: f32) -> f32 {
    let a = v.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f32::consts::LN_2
}

/// which band stays clean (unsaturated): LOWS, NONE (= saturate everything), HIGHS
pub const KEEP_NAMES: [&str; 3] = ["LOWS", "NONE", "HIGHS"];
const KEEP_LOW_HZ: f32 = 180.0;
const KEEP_HIGH_HZ: f32 = 2800.0;

#[derive(Clone)]
pub struct Sat {
    sr: f32,
    v1: [f32; 2],
    f1: [f32; 2],
    lp: [f32; 2],
    /// cached (amount, drive, bias, bias offset, makeup)
    k: (f32, f32, f32, f32, f32),
    dcx: [f32; 2],
    dcy: [f32; 2],
    dc_r: f32,
    a_lo: f32,
    a_hi: f32,
    /// AUTO GAIN: in / out power trackers (stereo), the applied gain and its target
    pin: f32,
    pout: f32,
    ag: f32,
    ag_t: f32,
    ag_a: f32,
    ag_s: f32,
    n: u32,
}

impl Sat {
    pub fn new(sr: f32) -> Self {
        let a = |f: f32| 1.0 - (-std::f32::consts::TAU * f / sr).exp();
        Self { sr, v1: [0.0; 2], f1: [lncosh(0.0); 2], k: (0.0, 1.0, 0.0, 0.0, 1.0), lp: [0.0; 2], dcx: [0.0; 2], dcy: [0.0; 2], dc_r: (-std::f32::consts::TAU * 5.0 / sr).exp(), a_lo: a(KEEP_LOW_HZ), a_hi: a(KEEP_HIGH_HZ), pin: 0.0, pout: 0.0, ag: 1.0, ag_t: 1.0, ag_a: 1.0 - (-1.0 / (0.25 * sr)).exp(), ag_s: 1.0 - (-1.0 / (0.05 * sr)).exp(), n: 0 }
    }
    pub fn reset(&mut self) {
        // keep AUTO GAIN's learned level: hosts reset on play/stop, and starting again
        // from unity would make the first hit after play sound different from the rest
        let (pin, pout, ag, ag_t) = (self.pin, self.pout, self.ag, self.ag_t);
        *self = Self::new(self.sr);
        (self.pin, self.pout, self.ag, self.ag_t) = (pin, pout, ag, ag_t);
    }

    /// Drive, bias and output gain for an amount 0..1.
    #[inline]
    pub fn coefs(amt: f32) -> (f32, f32, f32) {
        let s = amt.clamp(0.0, 1.0);
        let g = 1.0 + 5.0 * s * s.sqrt();
        let b = 0.22 * s;
        // level-match a -12 dBFS peak
        let r = 0.25;
        let mk = r / ((g * r + b).tanh() - b.tanh());
        (g, b, mk)
    }

    #[inline]
    fn shape(&mut self, ch: usize, u: f32) -> f32 {
        let (_, g, b, bt, mk) = self.k;
        let v = g * u + b;
        let f = lncosh(v);
        let dv = v - self.v1[ch];
        let y = if dv.abs() > 1e-4 { (f - self.f1[ch]) / dv } else { (0.5 * (v + self.v1[ch])).tanh() };
        self.v1[ch] = v;
        self.f1[ch] = f;
        (y - bt) * mk
    }

    /// One stereo sample with AUTO GAIN: the output follows the input's loudness (power
    /// tracked over ~250 ms, gain glides over ~50 ms, limited to +-12 dB), so the knob
    /// changes the tone, not the level. Amount 0 is a bit-exact bypass.
    #[inline]
    pub fn tick2(&mut self, x0: f32, x1: f32, amt: f32, keep: i32) -> (f32, f32) {
        if amt <= 1e-4 {
            self.pin = 0.0;
            self.pout = 0.0;
            self.ag = 1.0;
            self.ag_t = 1.0;
            return (self.tick(0, x0, 0.0, keep), self.tick(1, x1, 0.0, keep));
        }
        let (y0, y1) = (self.tick(0, x0, amt, keep), self.tick(1, x1, amt, keep));
        self.pin += self.ag_a * (x0 * x0 + x1 * x1 - self.pin);
        self.pout += self.ag_a * (y0 * y0 + y1 * y1 - self.pout);
        self.n = (self.n + 1) % 16;
        if self.n == 0 {
            // follow only when there's signal, so tails and silence don't swing the gain
            if self.pin > 1e-8 {
                self.ag_t = (self.pin / self.pout.max(1e-12)).sqrt().clamp(0.25, 4.0);
            }
        }
        self.ag += self.ag_s * (self.ag_t - self.ag);
        (y0 * self.ag, y1 * self.ag)
    }

    /// One sample, one channel, no auto gain. `keep`: 0 LOW, 1 MID, 2 HIGH.
    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32, amt: f32, keep: i32) -> f32 {
        if amt <= 1e-4 {
            self.v1[ch] = 0.0;
            self.f1[ch] = lncosh(0.0);
            self.lp[ch] = x;
            return x;
        }
        if amt != self.k.0 {
            let (g, b, mk) = Self::coefs(amt);
            self.k = (amt, g, b, b.tanh(), mk);
        }
        let y = match keep {
            0 => {
                self.lp[ch] += self.a_lo * (x - self.lp[ch]);
                let lo = self.lp[ch];
                lo + self.shape(ch, x - lo)
            }
            2 => {
                self.lp[ch] += self.a_hi * (x - self.lp[ch]);
                let lo = self.lp[ch];
                (x - lo) + self.shape(ch, lo)
            }
            _ => self.shape(ch, x),
        };
        // the bias leaves DC: block it
        let o = y - self.dcx[ch] + self.dc_r * self.dcy[ch];
        self.dcx[ch] = y;
        self.dcy[ch] = o;
        o
    }
}

/// Smoothed RMS-ish level detector (stereo linked), in dB.
#[derive(Clone)]
pub struct Level {
    ms: f32,
    a: f32,
}
impl Level {
    pub fn new(sr: f32, tau_s: f32) -> Self {
        Self { ms: 0.0, a: 1.0 - (-1.0 / (tau_s * sr)).exp() }
    }
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> f32 {
        let p = (l * l).max(r * r);
        self.ms += self.a * (p - self.ms);
        // RMS of a sine sits 3 dB under its peak: add it back so thresholds read in peak dBFS
        10.0 * (2.0 * self.ms).max(1e-12).log10()
    }
}

/// Soft-knee downward gain computer: gain reduction (dB, >= 0) for a level.
#[inline]
pub fn gr_soft(level: f32, thresh: f32, ratio: f32, knee: f32) -> f32 {
    let over = level - thresh;
    let s = 1.0 - 1.0 / ratio.max(1.0);
    if 2.0 * over <= -knee {
        0.0
    } else if 2.0 * over < knee {
        s * (over + knee * 0.5).powi(2) / (2.0 * knee)
    } else {
        s * over
    }
}

#[inline]
pub fn coef(sr: f32, t: f32) -> f32 {
    (-1.0 / (t.max(1e-5) * sr)).exp()
}

/// ARC: crest factor (peak vs RMS of the input over ~300 ms) -> release multiplier.
/// Low crest (sustained 808, pads, held notes) releases slower so it doesn't pump or
/// distort the lows; high crest (snare, kick, plucks) releases faster so it's back up
/// before the next hit. Independent of threshold and input level.
#[derive(Clone)]
pub struct Crest {
    pk: f32,
    ms: f32,
    pk_rel: f32,
    ms_a: f32,
    m: f32,
}
pub const CREST_LO_DB: f32 = 4.0;
pub const CREST_HI_DB: f32 = 14.0;
impl Crest {
    pub fn new(sr: f32) -> Self {
        Self { pk: 0.0, ms: 0.0, pk_rel: coef(sr, 0.3), ms_a: 1.0 - coef(sr, 0.3), m: 1.0 }
    }
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) {
        let a = l.abs().max(r.abs());
        self.pk = a.max(self.pk * self.pk_rel);
        self.ms += self.ms_a * (a * a - self.ms);
    }
    /// Crest factor in dB (None in silence).
    pub fn db(&self) -> Option<f32> {
        (self.ms > 1e-10).then(|| 10.0 * (self.pk * self.pk / self.ms).max(1.0).log10())
    }
    /// Release multiplier: 2.5x at <= 4 dB crest .. 0.5x at >= 14 dB (log-interpolated).
    /// Holds its last value through silence.
    pub fn mult(&mut self) -> f32 {
        if let Some(c) = self.db() {
            let t = ((c - CREST_LO_DB) / (CREST_HI_DB - CREST_LO_DB)).clamp(0.0, 1.0);
            // 0.1.3: like RComp's ARC, sustained material keeps the knob's release (was 2.5x),
            // punchy material gets quicker (down to 0.4x)
            self.m = 0.4f32.powf(t);
        }
        self.m
    }
}
