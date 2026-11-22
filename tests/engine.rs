use dbcomp::*;
const SR: f32 = 48000.0;
fn sine(n: usize, db: f32, f: f32) -> Vec<f32> {
    let a = 10f32.powf(db / 20.0);
    (0..n).map(|i| a * (std::f32::consts::TAU * f * i as f32 / SR).sin()).collect()
}
fn noise(n: usize, a: f32) -> Vec<f32> {
    let mut s = 4242u32;
    (0..n).map(|_| { s = s.wrapping_mul(1664525).wrapping_add(1013904223); a * ((s >> 8) as f32 / 8388608.0 - 1.0) }).collect()
}
fn rms(x: &[f32]) -> f32 { (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt() }
fn run(st: &State, x: &[f32]) -> Vec<f32> {
    let mut e = Engine::new(SR);
    let (mut a, mut b) = (x.to_vec(), x.to_vec());
    for (p, q) in a.chunks_mut(256).zip(b.chunks_mut(256)) { e.process(p, q, st); }
    a
}
fn gain_db(st: &State, x: &[f32]) -> f32 {
    let y = run(st, x);
    20.0 * (rms(&y[x.len() / 2..]) / rms(&x[x.len() / 2..])).log10()
}
/// Goertzel magnitude at f (whole-cycle windows assumed)
fn mag(x: &[f32], f: f32) -> f32 {
    let w = std::f32::consts::TAU * f / SR;
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    let c = 2.0 * (w as f64).cos();
    for v in x { let s = *v as f64 + c * s1 - s2; s2 = s1; s1 = s; }
    (((s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0)).sqrt() * 2.0 / x.len() as f64) as f32
}
fn thd(y: &[f32], f: f32) -> f32 {
    let h: f32 = (2..=5).map(|k| mag(y, f * k as f32).powi(2)).sum::<f32>().sqrt();
    h / mag(y, f)
}

#[test]
fn defaults_are_transparent() {
    let x = noise(48000, 0.5);
    let y = run(&State { safe: false, ..State::default() }, &x);
    for i in 0..x.len() { assert_eq!(y[i], x[i], "sample {i}"); }
}

#[test]
fn static_curve_matches_thresh_and_ratio() {
    let x = sine(48000, -10.0, 200.0);
    for (th, ra, want) in [(-20.0f32, 4.0f32, -7.5f32), (-20.0, 2.0, -5.0), (-30.0, 10.0, -18.0), (0.0, 4.0, 0.0)] {
        let g = gain_db(&State { thresh: th, ratio: ra, warm: false, safe: false, ..State::default() }, &x);
        println!("thresh {th} ratio {ra}: {g:.2} dB (want {want})");
        assert!((g - want).abs() < 0.8, "{th}/{ra}: {g}");
    }
}

#[test]
fn slow_attack_lets_the_snare_crack_through() {
    // quiet bed, then a hit: compare the peak of the first 4 ms of the hit
    let mut x = sine(24000, -40.0, 180.0);
    x.extend(sine(4800, -6.0, 180.0));
    let first = |at: f32| {
        let y = run(&State { thresh: -30.0, ratio: 8.0, attack_ms: at, warm: false, safe: false, ..State::default() }, &x);
        y[24000..24000 + 192].iter().fold(0.0f32, |a, v| a.max(v.abs()))
    };
    let (fast, slow) = (first(0.5), first(40.0));
    println!("first 4 ms peak: attack 0.5 ms {:.1} dB, 40 ms {:.1} dB", 20.0 * fast.log10(), 20.0 * slow.log10());
    assert!(slow > fast * 1.6);
}

/// dB of GR left 150 ms after a loud section ends
fn tail(arc: bool, opto: bool, loud_s: f32) -> f32 {
    let n = (loud_s * SR) as usize;
    let mut x = sine(n, -6.0, 150.0);
    x.extend(sine(24000, -40.0, 150.0));
    let st = State { thresh: -30.0, ratio: 6.0, release_ms: 100.0, arc, opto, warm: false, safe: false, ..State::default() };
    let y = run(&st, &x);
    let i = n + 7200;
    let w = 960;
    -20.0 * (rms(&y[i..i + w]) / rms(&x[i..i + w])).log10()
}

#[test]
fn release_follows_arc_and_behaviour() {
    let (e, o) = (tail(false, false, 0.3), tail(false, true, 0.3));
    let (short, long) = (tail(true, false, 0.05), tail(true, false, 2.0));
    println!("GR 150 ms after: electro {e:.1}, opto {o:.1}; ARC after 50 ms burst {short:.1}, after 2 s {long:.1}");
    assert!((e - o).abs() > 0.5, "OPTO and ELECTRO should release differently");
    assert!(long > short + 0.5, "ARC: sustained compression releases slower");
}

/// ARC by crest factor: a sustained tone (crest 3 dB) releases slower than spiky drums
/// (crest > 14 dB) at the same settings.
#[test]
fn arc_reads_the_crest_factor() {
    let sr = SR as usize;
    let tone = sine(sr, -10.0, 60.0);
    let mut s = 7u32;
    let drums: Vec<f32> = (0..sr).map(|i| {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        let t = (i % 12000) as f32 / SR;
        0.9 * (-t * 60.0).exp() * ((s >> 8) as f32 / 8388608.0 - 1.0)
    }).collect();
    let m = |x: &[f32]| {
        let mut c = dbcore::Crest::new(SR);
        for v in x { c.tick(*v, *v); }
        (c.db().unwrap(), c.mult())
    };
    let ((ct, mt), (cd, md)) = (m(&tone), m(&drums));
    println!("crest: tone {ct:.1} dB -> release x{mt:.2}; drums {cd:.1} dB -> release x{md:.2}");
    assert!(ct < 4.5 && mt > 2.0);
    assert!(cd > 12.0 && md < 0.8);
}

#[test]
fn sat_zero_is_bit_exact_and_sat_adds_harmonics_not_level() {
    let x = sine(48000, -12.0, 100.0);
    let y0 = run(&State { sat: 0.0, safe: false, ..State::default() }, &x);
    assert_eq!(y0, x);
    let seg = 24000..48000;
    for s in [0.25f32, 0.5, 1.0] {
        let y = run(&State { sat: s, safe: false, ..State::default() }, &x);
        let lvl = 20.0 * (rms(&y[seg.clone()]) / rms(&x[seg.clone()])).log10();
        let t = thd(&y[seg.clone()], 100.0);
        let h2 = mag(&y[seg.clone()], 200.0) / mag(&y[seg.clone()], 100.0);
        println!("SAT {:.0}%: level {lvl:+.2} dB, THD {:.2}%, 2nd {:.2}%", s * 100.0, t * 100.0, h2 * 100.0);
        assert!(lvl.abs() < 0.5, "AUTO GAIN: level jump {lvl}");
        assert!(t > 0.004 * s && h2 > 0.001, "too clean");
    }
    let a = thd(&run(&State { sat: 0.25, ..State::default() }, &x)[seg.clone()], 100.0);
    let b = thd(&run(&State { sat: 1.0, ..State::default() }, &x)[seg.clone()], 100.0);
    assert!(b > a * 2.0, "more knob, more harmonics");
}

#[test]
fn keep_low_leaves_the_bass_clean() {
    let x = sine(48000, -9.0, 50.0);
    let seg = 24000..48000;
    let t = |k: i32| thd(&run(&State { sat: 0.8, keep: k, safe: false, ..State::default() }, &x)[seg.clone()], 50.0);
    let (lo, mid, hi) = (t(0), t(1), t(2));
    println!("50 Hz THD: KEEP LOW {:.2}%, MID {:.2}%, HIGH {:.2}%", lo * 100.0, mid * 100.0, hi * 100.0);
    assert!(lo < mid * 0.4);
    let x = sine(48000, -9.0, 6000.0);
    let t = |k: i32| thd(&run(&State { sat: 0.8, keep: k, safe: false, ..State::default() }, &x)[seg.clone()], 6000.0);
    assert!(t(2) < t(1) * 0.5, "KEEP HIGH leaves the top clean");
}

#[test]
fn safe_never_over_zero() {
    let x = noise(48000, 1.0);
    let y = run(&State { gain_db: 18.0, sat: 1.0, ..State::default() }, &x);
    let pk = y.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    assert!(pk <= 0.9886, "{pk}");
}

#[test]
fn mix_zero_and_bypass_are_dry() {
    let x = noise(48000, 0.5);
    for st in [State { thresh: -30.0, sat: 1.0, mix: 0.0, safe: false, ..State::default() }, State { thresh: -30.0, sat: 1.0, bypass: true, safe: false, ..State::default() }] {
        let y = run(&st, &x);
        for i in 4800..x.len() { assert!((y[i] - x[i]).abs() < 1e-5); }
    }
}

#[test]
fn cpu() {
    let x = noise(48000 * 10, 0.5);
    let st = State { thresh: -24.0, sat: 0.5, ..State::default() };
    let mut e = Engine::new(SR);
    let (mut a, mut b) = (x.clone(), x.clone());
    let t = std::time::Instant::now();
    for (p, q) in a.chunks_mut(256).zip(b.chunks_mut(256)) { e.process(p, q, &st); }
    let pct = t.elapsed().as_secs_f32() / 10.0 * 100.0;
    println!("dbcomp: {pct:.3}% of one core (stereo, 48 kHz)");
    assert!(pct < 1.0);
}
