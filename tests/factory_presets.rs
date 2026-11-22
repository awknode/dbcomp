//! Factory presets: validated against the real params, played (bounded), written with PRESET_OUT.
use nice_plug::prelude::*;
use serde_json::{json, Map, Value};
use dbcomp::*;

type Preset = (&'static str, &'static [(&'static str, f64)]);
const PRESETS: &[Preset] = &[
    ("SNARE - Crack", &[("thresh", -22.0), ("ratio", 4.0), ("attack", 18.0), ("release", 90.0), ("gain", 4.0), ("sat", 0.2)]),
    ("SNARE - Fat + Dirty", &[("thresh", -26.0), ("ratio", 6.0), ("attack", 8.0), ("release", 120.0), ("gain", 6.0), ("sat", 0.55)]),
    ("SNARE - Tame Rimshots", &[("thresh", -14.0), ("ratio", 8.0), ("attack", 0.8), ("release", 60.0), ("gain", 2.0)]),
    ("KICK - Punch", &[("thresh", -18.0), ("ratio", 4.0), ("attack", 25.0), ("release", 70.0), ("gain", 3.0), ("sat", 0.2), ("keep", 0.0)]),
    ("KICK - Tight", &[("thresh", -20.0), ("ratio", 6.0), ("attack", 3.0), ("release", 50.0), ("gain", 3.0)]),
    ("DRUMS - Bus Glue", &[("thresh", -16.0), ("ratio", 2.0), ("attack", 30.0), ("release", 200.0), ("gain", 2.0), ("opto", 1.0), ("sat", 0.15)]),
    ("DRUMS - Room Smash", &[("thresh", -32.0), ("ratio", 12.0), ("attack", 1.0), ("release", 250.0), ("gain", 8.0), ("sat", 0.4)]),
    ("HATS - Even Out", &[("thresh", -20.0), ("ratio", 3.0), ("attack", 2.0), ("release", 60.0), ("gain", 2.0), ("warm", 0.0)]),
    ("808 - Hold It Steady", &[("thresh", -16.0), ("ratio", 4.0), ("attack", 20.0), ("release", 300.0), ("gain", 2.0), ("sat", 0.3), ("keep", 0.0)]),
    ("BASS - Level", &[("thresh", -20.0), ("ratio", 4.0), ("attack", 10.0), ("release", 150.0), ("gain", 3.0), ("opto", 1.0), ("sat", 0.2)]),
    ("VOCAL - Smooth Opto", &[("thresh", -20.0), ("ratio", 3.0), ("attack", 8.0), ("release", 180.0), ("gain", 4.0), ("opto", 1.0)]),
    ("VOCAL - Rap Upfront", &[("thresh", -24.0), ("ratio", 6.0), ("attack", 3.0), ("release", 80.0), ("gain", 6.0), ("sat", 0.3), ("keep", 2.0)]),
    ("GUITAR - Even Strums", &[("thresh", -18.0), ("ratio", 3.0), ("attack", 15.0), ("release", 150.0), ("gain", 3.0), ("opto", 1.0)]),
    ("KEYS - Gentle Glue", &[("thresh", -14.0), ("ratio", 2.0), ("attack", 20.0), ("release", 250.0), ("gain", 1.5), ("opto", 1.0)]),
    ("SAT ONLY - Warm Touch", &[("sat", 0.3)]),
    ("SAT ONLY - Drive It", &[("sat", 0.75)]),
    ("MIX BUS - 2 dB Glue", &[("thresh", -10.0), ("ratio", 1.5), ("attack", 30.0), ("release", 300.0), ("gain", 1.0), ("opto", 1.0), ("warm", 0.0)]),
];

fn build(params: &DbCompParams, over: &[(&str, f64)]) -> Value {
    let map = params.param_map();
    let mut out = Map::new();
    for (id, v) in over {
        let ptr = map.iter().find(|(i, _, _)| i == id).unwrap_or_else(|| panic!("no param {id}")).1;
        let val = unsafe {
            match ptr {
                ParamPtr::FloatParam(p) => {
                    let p = &*p;
                    assert!((p.preview_plain(p.preview_normalized(*v as f32)) - *v as f32).abs() < 1e-3, "{id} out of range");
                    json!({ "f32": *v as f32 })
                }
                ParamPtr::IntParam(p) => {
                    let p = &*p;
                    assert_eq!(p.preview_plain(p.preview_normalized(*v as i32)), *v as i32, "{id} out of range");
                    json!({ "i32": *v as i32 })
                }
                ParamPtr::BoolParam(_) => json!({ "bool": *v != 0.0 }),
                _ => panic!("{id}"),
            }
        };
        out.insert(id.to_string(), val);
    }
    json!({ "version": env!("CARGO_PKG_VERSION"), "params": out, "fields": {} })
}

#[test]
fn factory_presets_are_valid() {
    let params = DbCompParams::default();
    let dir = std::env::var("PRESET_OUT").ok();
    for p in PRESETS {
        assert!(!p.0.contains('/'));
        let v = build(&params, p.1);
        let _: nice_plug::plugin::PluginState = serde_json::from_value(v.clone()).unwrap();
        let mut st = State::default();
        for (id, x) in p.1 {
            match *id {
                "thresh" => st.thresh = *x as f32,
                "ratio" => st.ratio = *x as f32,
                "attack" => st.attack_ms = *x as f32,
                "release" => st.release_ms = *x as f32,
                "gain" => st.gain_db = *x as f32,
                "sat" => st.sat = *x as f32,
                "keep" => st.keep = *x as i32,
                "opto" => st.opto = *x != 0.0,
                "warm" => st.warm = *x != 0.0,
                "arc" => st.arc = *x != 0.0,
                k => panic!("{k}"),
            }
        }
        let mut e = Engine::new(48000.0);
        let mut s = 9u32;
        let mut l: Vec<f32> = (0..48000).map(|i| { s = s.wrapping_mul(1664525).wrapping_add(1013904223); 0.6 * ((s >> 8) as f32 / 8388608.0 - 1.0) * if (i / 5000) % 2 == 0 { 1.0 } else { 0.05 } }).collect();
        let mut r = l.clone();
        for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) { e.process(a, b, &st); }
        let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(pk.is_finite() && pk <= 1.0, "{}: {pk}", p.0);
        if let Some(d) = &dir {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(format!("{d}/{}.awkp", p.0), serde_json::to_string_pretty(&v).unwrap()).unwrap();
        }
    }
}
