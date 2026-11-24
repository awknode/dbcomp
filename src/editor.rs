//! dBcomp editor: dB series look (graphite, one accent). Five faders (THRESH, RATIO,
//! ATTACK, RELEASE, GAIN), a GR + OUT meter pair, the SAT knob with KEEP LOW / MID / HIGH,
//! and a footer with ARC, ELECTRO / OPTO, WARM / SMOOTH and SAFE.

use crate::awk_ui::*;
use crate::presets::PresetBar;
use crate::{get, DbCompParams, Shared};
use nice_plug::prelude::*;
use nice_plug_egui::create_egui_editor;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke};
use std::sync::Arc;

pub const WIDTH: u32 = 820;
pub const HEIGHT: u32 = 430;
const TOP: f32 = 44.0;

const BG: Color32 = rgb(0x14, 0x15, 0x16);
const PANEL: Color32 = rgb(0x1c, 0x1d, 0x1f);
const WELL: Color32 = rgb(0x0b, 0x0b, 0x0c);
const EDGE: Color32 = rgb(0x2c, 0x2e, 0x31);
const TXT: Color32 = rgb(0xec, 0xeb, 0xe6);
const DIM: Color32 = rgb(0x8b, 0x8a, 0x85);
const FAINT: Color32 = rgb(0x4a, 0x4a, 0x47);
const YEL: Color32 = rgb(0x2f, 0xd8, 0xc2); // the accent (teal on dBcomp)
const RED: Color32 = rgb(0xff, 0x4a, 0x3d);
const OFF: Color32 = rgb(0x26, 0x27, 0x29);
const NAME: &str = "dBcomp";

pub(crate) struct St {
    presets: PresetBar,
    skin: Skin,
}
fn new_state(name: &'static str) -> St {
    St { presets: PresetBar::new(name), skin: Skin::new(Look::Machined, YEL, "dBcomp", env!("CARGO_PKG_VERSION"), "") }
}

pub(crate) fn create(params: Arc<DbCompParams>, shared: Arc<Shared>) -> Option<crate::nice_app::AwkEditor> {
    let state = params.editor_state.clone();
    crate::presets::apply_scale(&state, &params.ui_scale);
    create_egui_editor(state, nice_plug_egui::RepaintNotifier::new(), nice_plug_egui::EguiNiceSettings::default(), crate::nice_app::AwkApp::new(install_fonts, {
        let mut __st = new_state("dBcomp");
        Box::new(move |ctx: &mut egui::Ui, setter: &ParamSetter| {
            let st = &mut __st;
            frame(ctx, setter, st, params.as_ref(), shared.as_ref())
        })
    }))
}

fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}
fn set_bool(setter: &ParamSetter, p: &BoolParam, v: bool) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
use egui::Painter;

fn pill(ui: &mut egui::Ui, r: Rect, s: &str, on: bool, id: impl std::hash::Hash + std::fmt::Debug) -> Response {
    let resp = ui.interact(r, ui.scope_id().with(("pill", id)), Sense::click());
    let pa = ui.painter();
    pa.rect_filled(r, r.height() * 0.5, if on { YEL } else if resp.hovered() { rgb(0x2a, 0x2b, 0x2e) } else { PANEL });
    pa.rect_stroke(r, r.height() * 0.5, Stroke::new(1.0, if on { YEL } else { EDGE }), egui::StrokeKind::Inside);
    txt(pa, r.center(), Align2::CENTER_CENTER, s, labb(12.0), if on { BG } else { DIM }, 1.5);
    resp
}

/// A lift fader: recessed slot, dB scale, a yellow fill up to the cap, a chunky cap.
/// `marks`: (plain value, label). Drag = move, Shift = fine, double-click = default, wheel = nudge.
#[allow(clippy::too_many_arguments)]
fn fader(ui: &mut egui::Ui, setter: &ParamSetter, param: &FloatParam, cx: f32, top: f32, bot: f32, label: &str, marks: &[(f32, &str)], from_mid: bool, fmt: &dyn Fn(f32) -> String) -> Response {
    let col = Rect::from_min_max(pos2(cx - 40.0, top - 14.0), pos2(cx + 40.0, bot + 14.0));
    let id = ui.scope_id().with(("fader", label));
    let resp = ui.interact(col, id, Sense::click_and_drag());
    let span = bot - top;
    let mut v = param.unmodulated_normalized_value();
    if resp.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
        v = param.default_normalized_value();
    } else {
        if resp.drag_started() {
            setter.begin_set_parameter(param);
        }
        if resp.dragged() {
            let fine = ui.input(|i| i.modifiers.shift);
            let dy = resp.drag_delta().y;
            v = (v - dy / span * if fine { 0.15 } else { 1.0 }).clamp(0.0, 1.0);
            setter.set_parameter_normalized(param, v);
        }
        if resp.drag_stopped() {
            setter.end_set_parameter(param);
        }
        if resp.hovered() {
            let w = ui.input(|i| i.events.iter().map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 }).sum::<f32>());
            if w != 0.0 {
                v = (v + w.signum() * 0.01).clamp(0.0, 1.0);
                setter.begin_set_parameter(param);
                setter.set_parameter_normalized(param, v);
                setter.end_set_parameter(param);
            }
        }
    }
    let hot = resp.hovered() || resp.dragged();
    let pa = ui.painter();
    let y_of = |n: f32| bot - n * span;
    // scale
    for (val, s) in marks {
        let y = y_of(param.preview_normalized(*val));
        pa.line_segment([pos2(cx - 30.0, y), pos2(cx - 20.0, y)], Stroke::new(1.0, FAINT));
        pa.line_segment([pos2(cx + 20.0, y), pos2(cx + 30.0, y)], Stroke::new(1.0, FAINT));
        txt(pa, pos2(cx - 34.0, y), Align2::RIGHT_CENTER, *s, mono(9.0), DIM, 0.0);
    }
    // slot
    let slot = Rect::from_min_max(pos2(cx - 4.0, top), pos2(cx + 4.0, bot));
    pa.rect_filled(slot.expand(2.0), 5.0, rgb(0x08, 0x08, 0x09));
    pa.rect_filled(slot, 4.0, WELL);
    let y = y_of(v);
    let base = if from_mid { y_of(param.preview_normalized(0.0)) } else { bot };
    let fill = Rect::from_min_max(pos2(cx - 3.0, y.min(base)), pos2(cx + 3.0, y.max(base)));
    if fill.height() > 0.5 {
        pa.rect_filled(fill.expand2(vec2(5.0, 0.0)), 4.0, alpha(YEL, 0.12));
        pa.rect_filled(fill, 3.0, YEL);
    }
    // cap
    let cap = Rect::from_center_size(pos2(cx, y), vec2(56.0, 30.0));
    for k in 0..5 {
        pa.rect_filled(cap.translate(vec2(0.0, 3.0 + k as f32)).expand(k as f32), 6.0, Color32::from_black_alpha(26));
    }
    pa.rect_filled(cap, 5.0, if hot { rgb(0x3a, 0x3b, 0x3e) } else { rgb(0x2e, 0x2f, 0x32) });
    pa.rect_filled(Rect::from_min_max(cap.min, pos2(cap.right(), cap.top() + 12.0)), egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, rgb(0x44, 0x45, 0x48));
    pa.rect_stroke(cap, 5.0, Stroke::new(1.0, Color32::from_white_alpha(24)), egui::StrokeKind::Inside);
    for gy in [-6.0, -2.0, 2.0, 6.0] {
        pa.line_segment([pos2(cap.left() + 8.0, cap.center().y + gy), pos2(cap.right() - 8.0, cap.center().y + gy)], Stroke::new(1.0, Color32::from_black_alpha(90)));
    }
    pa.rect_filled(Rect::from_center_size(cap.center(), vec2(cap.width() - 4.0, 2.0)), 1.0, YEL);
    // label + value
    txt(pa, pos2(cx, top - 30.0), Align2::CENTER_CENTER, label, labb(14.0), if hot { TXT } else { DIM }, 2.5);
    let s = fmt(param.value());
    txt(pa, pos2(cx, bot + 30.0), Align2::CENTER_CENTER, s, mono(13.0), YEL, 0.0);
    resp
}

/// Vertical meter, 30 segments. `down`: fills from the top (gain reduction).
fn bar(pa: &Painter, r: Rect, u: f32, col: Color32, label: &str, down: bool) {
    pa.rect_filled(r, 3.0, WELL);
    let n = 30;
    let h = r.height() / n as f32;
    for i in 0..n {
        let p = (i as f32 + 0.5) / n as f32;
        let (a, b) = if down { (r.top() + i as f32 * h + 1.0, r.top() + (i + 1) as f32 * h - 1.0) } else { (r.bottom() - (i + 1) as f32 * h + 1.0, r.bottom() - i as f32 * h - 1.0) };
        pa.rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, a), pos2(r.right() - 2.0, b)), 1.0, if p <= u { col } else { OFF });
    }
    txt(pa, pos2(r.center().x, r.bottom() + 14.0), Align2::CENTER_CENTER, label, labb(11.0), DIM, 1.5);
}

/// The SAT knob: 270° sweep, accent arc, pointer, value under it.
fn sat_knob(ui: &mut egui::Ui, setter: &ParamSetter, param: &FloatParam, c: Pos2, rad: f32) -> Response {
    let id = ui.scope_id().with("sat");
    let resp = ui.interact(Rect::from_center_size(c, vec2(rad * 2.4, rad * 2.4)), id, Sense::click_and_drag());
    knob_input(ui, setter, &resp, id, param);
    let v = param.unmodulated_normalized_value();
    let hot = resp.hovered() || resp.dragged();
    let pa = ui.painter();
    let a0 = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let pt = |a: f32, r: f32| c + vec2(a.cos(), a.sin()) * r;
    let arc = |t0: f32, t1: f32, r: f32| -> Vec<Pos2> { (0..=48).map(|k| pt(a0 + sweep * (t0 + (t1 - t0) * k as f32 / 48.0), r)).collect() };
    pa.add(Shape::line(arc(0.0, 1.0, rad + 10.0), Stroke::new(5.0, OFF)));
    if v > 0.002 {
        pa.add(Shape::line(arc(0.0, v, rad + 10.0), Stroke::new(9.0, alpha(YEL, 0.15))));
        pa.add(Shape::line(arc(0.0, v, rad + 10.0), Stroke::new(5.0, YEL)));
    }
    for k in 0..5 {
        pa.circle_filled(c + vec2(0.0, 3.0 + k as f32), rad + k as f32, Color32::from_black_alpha(22));
    }
    pa.circle_filled(c, rad, if hot { rgb(0x3a, 0x3b, 0x3e) } else { rgb(0x2e, 0x2f, 0x32) });
    pa.circle_filled(c - vec2(0.0, rad * 0.25), rad * 0.72, rgb(0x36, 0x37, 0x3a));
    pa.circle_stroke(c, rad, Stroke::new(1.0, Color32::from_white_alpha(24)));
    let a = a0 + sweep * v;
    pa.line_segment([pt(a, rad * 0.35), pt(a, rad * 0.85)], Stroke::new(4.0, YEL));
    resp
}

fn frame(root: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &DbCompParams, sh: &Shared) {
    let ctx_owned = root.ctx().clone();
    let ctx = &ctx_owned;
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
    st.skin.begin(ctx);
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG)).show(root, |ui| {
        let whole = ui.max_rect();
        let o = whole.min.to_vec2() + vec2(30.0, TOP);
        let at = |x: f32, y: f32| pos2(x, y) + o;
        let pa = ui.painter().clone();
        // header
        let w = txt(&pa, at(18.0, 32.0), Align2::LEFT_CENTER, "dB", labb(36.0), YEL, 2.0).width();
        txt(&pa, at(18.0 + w + 2.0, 32.0), Align2::LEFT_CENTER, "COMP", labb(36.0), TXT, 5.0);
        txt(&pa, at(190.0, 26.0), Align2::LEFT_CENTER, "EVERYDAY COMPRESSOR", mono(10.0), YEL, 1.5);
        txt(&pa, at(190.0, 40.0), Align2::LEFT_CENTER, "ONE-KNOB SAT // ZERO LATENCY", mono(10.0), DIM, 1.5);
        let gr = get(&sh.gr);
        txt(&pa, at(760.0, 32.0), Align2::RIGHT_CENTER, format!("GR {:.1} dB", gr), mono(13.0), if gr > 0.05 { RED } else { DIM }, 0.5);
        // fader deck
        let deck = Rect::from_min_max(at(16.0, 60.0), at(470.0, 340.0));
        pa.rect_filled(deck, 10.0, PANEL);
        pa.rect_stroke(deck, 10.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
        let (top, bot) = (o.y + 116.0, o.y + 290.0);
        let db1 = |v: f32| format!("{:.1} dB", v);
        let ms = |v: f32| if v < 10.0 { format!("{:.1} ms", v) } else { format!("{:.0} ms", v) };
        let tm = [(-48.0, "-48"), (-36.0, "-36"), (-24.0, "-24"), (-12.0, "-12"), (0.0, "0")];
        let rm = [(1.0, "1"), (2.0, "2"), (4.0, "4"), (8.0, "8"), (20.0, "20")];
        let am = [(0.5, ".5"), (5.0, "5"), (30.0, "30"), (200.0, "200")];
        let lm = [(5.0, "5"), (50.0, "50"), (300.0, "300"), (2000.0, "2k")];
        let gm = [(-18.0, "-18"), (-6.0, "-6"), (0.0, "0"), (6.0, "+6"), (18.0, "+18")];
        fader(ui, setter, &p.thresh, o.x + 68.0, top, bot, "THRESH", &tm, false, &db1).on_hover_text("Level where compression starts. Double-click: 0 dB (off)");
        fader(ui, setter, &p.ratio, o.x + 152.0, top, bot, "RATIO", &rm, false, &|v: f32| format!("{:.1}:1", v)).on_hover_text("How hard it compresses above the threshold. Double-click: 4:1");
        fader(ui, setter, &p.attack, o.x + 236.0, top, bot, "ATTACK", &am, false, &ms).on_hover_text("How fast it clamps down. Slower lets the snare crack through. Double-click: 10 ms");
        fader(ui, setter, &p.release, o.x + 320.0, top, bot, "RELEASE", &lm, false, &ms).on_hover_text("How fast it lets go (ARC adapts this to the material). Double-click: 120 ms");
        fader(ui, setter, &p.gain, o.x + 404.0, top, bot, "GAIN", &gm, true, &|v: f32| format!("{:+.1} dB", v)).on_hover_text("Makeup / output gain. Double-click: 0 dB");
        // meters
        let mp = Rect::from_min_max(at(482.0, 60.0), at(596.0, 340.0));
        pa.rect_filled(mp, 10.0, PANEL);
        pa.rect_stroke(mp, 10.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
        let m = &sh.meters;
        let (bt, bb) = (o.y + 84.0, o.y + 306.0);
        bar(&pa, Rect::from_min_max(pos2(o.x + 500.0, bt), pos2(o.x + 528.0, bb)), (gr / 24.0).clamp(0.0, 1.0), RED, "GR", true);
        bar(&pa, Rect::from_min_max(pos2(o.x + 550.0, bt), pos2(o.x + 578.0, bb)), ((m.db(2).max(m.db(3)) + 48.0) / 54.0).clamp(0.0, 1.0), YEL, "OUT", false);
        // SAT
        let sp = Rect::from_min_max(at(608.0, 60.0), at(760.0, 340.0));
        pa.rect_filled(sp, 10.0, PANEL);
        pa.rect_stroke(sp, 10.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
        txt(&pa, at(684.0, 86.0), Align2::CENTER_CENTER, "SAT", labb(14.0), DIM, 2.5);
        sat_knob(ui, setter, &p.sat, at(684.0, 170.0), 40.0).on_hover_text("One-knob saturation. 0 = off. Double-click: 0");
        txt(&pa, at(684.0, 238.0), Align2::CENTER_CENTER, format!("{:.0}%", p.sat.value() * 100.0), mono(13.0), YEL, 0.0);
        txt(&pa, at(684.0, 266.0), Align2::CENTER_CENTER, "CLEAN", mono(9.0), DIM, 1.5);
        let keep = p.keep.value();
        let tips = ["Lows (under ~180 Hz) stay clean, the rest is saturated", "Nothing kept clean: saturate the whole signal", "Highs (over ~2.8 kHz) stay clean, the body is saturated"];
        for (k, nm) in crate::dbcore::KEEP_NAMES.iter().enumerate() {
            let r = Rect::from_center_size(at(638.0 + k as f32 * 46.0, 290.0), vec2(42.0, 20.0));
            if pill(ui, r, nm, keep == k as i32, ("keep", k)).on_hover_text(tips[k]).clicked() {
                setter.begin_set_parameter(&p.keep);
                setter.set_parameter(&p.keep, k as i32);
                setter.end_set_parameter(&p.keep);
            }
        }
        // footer
        let fr = Rect::from_min_max(pos2(whole.left(), o.y + 352.0), whole.right_bottom());
        pa.rect_filled(fr, 0.0, rgb(0x0e, 0x0e, 0x0f));
        pa.line_segment([fr.left_top(), fr.right_top()], Stroke::new(1.0, EDGE));
        let cy = fr.center().y;
        let arc = p.arc.value();
        if pill(ui, Rect::from_center_size(pos2(o.x + 40.0, cy), vec2(60.0, 20.0)), "ARC", arc, "arc").on_hover_text("Auto release: longer under sustained compression, quicker on short peaks").clicked() {
            set_bool(setter, &p.arc, !arc);
        }
        let opto = p.opto.value();
        if pill(ui, Rect::from_center_size(pos2(o.x + 120.0, cy), vec2(86.0, 20.0)), if opto { "OPTO" } else { "ELECTRO" }, true, "beh").on_hover_text("ELECTRO: lets go fast near 0 dB (punchy). OPTO: lets go fast from deep GR, then eases in. Click to switch").clicked() {
            set_bool(setter, &p.opto, !opto);
        }
        let warm = p.warm.value();
        if pill(ui, Rect::from_center_size(pos2(o.x + 214.0, cy), vec2(86.0, 20.0)), if warm { "WARM" } else { "SMOOTH" }, true, "warm").on_hover_text("WARM: a little colour that grows with the compression. SMOOTH: clean. Click to switch").clicked() {
            set_bool(setter, &p.warm, !warm);
        }
        let safe = p.safe.value();
        let lim = -m.db(4);
        let ls = if !safe { "SAFE OFF".to_string() } else if lim > 0.1 { format!("SAFE -{:.1}", lim) } else { "SAFE 0 dB".to_string() };
        if pill(ui, Rect::from_center_size(pos2(o.x + 316.0, cy), vec2(96.0, 20.0)), &ls, safe, "safe").on_hover_text("Never over 0 dBFS").clicked() {
            set_bool(setter, &p.safe, !safe);
        }
        txt(&pa, pos2(o.x + 380.0, cy), Align2::LEFT_CENTER, "LATENCY 0", mono(10.0), DIM, 1.5);
        txt(&pa, pos2(o.x + 760.0, cy), Align2::RIGHT_CENTER, format!("awk studios · {} v{}", NAME.to_lowercase(), env!("CARGO_PKG_VERSION")), mono(10.0), DIM, 1.0);
        let _ = st.presets.strip(ui, setter, p, whole, &st.skin, &p.mix, &p.bypass, &p.ui_scale);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, State};
    use nice_plug::context::gui::GuiContextInner;
    use nice_plug::plugin::PluginState;
    use egui::{epaint, Event, ImageData, Modifiers, PointerButton, RawInput, TextureId, TextureWrapMode, Vec2};
    use std::collections::HashMap;

    struct Mock;
    impl GuiContextInner for Mock {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Vst3
        }
        fn request_restart(&self) {}
        unsafe fn raw_begin_set_parameter(&self, _p: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _p: ParamPtr, _n: f32) {}
        unsafe fn raw_end_set_parameter(&self, _p: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState { version: String::new(), params: Default::default(), fields: Default::default() }
        }
        fn set_state(&self, _s: PluginState) {}
    }
    struct Tex {
        w: usize,
        h: usize,
        px: Vec<[f32; 4]>,
        repeat: bool,
    }
    fn apply_delta(texs: &mut HashMap<TextureId, Tex>, out: &egui::FullOutput) {
        for (id, d) in out.textures_delta.set.iter().flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
            let (w, h, px): (usize, usize, Vec<[f32; 4]>) = match &d.image {
                ImageData::Color(c) => (c.size[0], c.size[1], c.pixels.iter().map(|p| [p.r() as f32 / 255.0, p.g() as f32 / 255.0, p.b() as f32 / 255.0, p.a() as f32 / 255.0]).collect()),
            };
            let repeat = d.options.wrap_mode == TextureWrapMode::Repeat;
            if let Some(pos) = d.pos {
                let t = texs.get_mut(id).unwrap();
                for y in 0..h {
                    for x in 0..w {
                        t.px[(pos[1] + y) * t.w + pos[0] + x] = px[y * w + x];
                    }
                }
            } else {
                texs.insert(*id, Tex { w, h, px, repeat });
            }
        }
    }
    fn render(ctx: &egui::Context, out: egui::FullOutput, texs: &mut HashMap<TextureId, Tex>, path: &str) {
        let img = raster(ctx, out, texs);
        let bytes: Vec<u8> = img.iter().flat_map(|p| p.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8).collect::<Vec<_>>()).collect();
        std::fs::write(path, bytes).unwrap();
    }
    fn raster(ctx: &egui::Context, out: egui::FullOutput, texs: &mut HashMap<TextureId, Tex>) -> Vec<[f32; 3]> {
        apply_delta(texs, &out);
        let (w, h) = (WIDTH as usize, HEIGHT as usize);
        let mut img = vec![[0.0f32; 3]; w * h];
        let prims = ctx.tessellate(out.shapes, 1.0);
        for cp in prims {
            let epaint::Primitive::Mesh(mesh) = cp.primitive else { continue };
            let clip = cp.clip_rect;
            let tex = texs.get(&mesh.texture_id);
            for tri in mesh.indices.chunks(3) {
                let v = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
                let (x0, x1) = (v.iter().map(|q| q.pos.x).fold(f32::MAX, f32::min).max(clip.left()).max(0.0), v.iter().map(|q| q.pos.x).fold(f32::MIN, f32::max).min(clip.right()).min(w as f32));
                let (y0, y1) = (v.iter().map(|q| q.pos.y).fold(f32::MAX, f32::min).max(clip.top()).max(0.0), v.iter().map(|q| q.pos.y).fold(f32::MIN, f32::max).min(clip.bottom()).min(h as f32));
                let area = (v[1].pos.x - v[0].pos.x) * (v[2].pos.y - v[0].pos.y) - (v[2].pos.x - v[0].pos.x) * (v[1].pos.y - v[0].pos.y);
                if area.abs() < 1e-6 {
                    continue;
                }
                for py in (y0.floor() as usize)..(y1.ceil() as usize).min(h) {
                    for px in (x0.floor() as usize)..(x1.ceil() as usize).min(w) {
                        let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                        let e = |a: &epaint::Vertex, b: &epaint::Vertex| (b.pos.x - a.pos.x) * (fy - a.pos.y) - (fx - a.pos.x) * (b.pos.y - a.pos.y);
                        let (w0, w1, w2) = (e(&v[1], &v[2]) / area, e(&v[2], &v[0]) / area, e(&v[0], &v[1]) / area);
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let mut c = [0.0f32; 4];
                        for i in 0..4 {
                            c[i] = (w0 * v[0].color.to_array()[i] as f32 + w1 * v[1].color.to_array()[i] as f32 + w2 * v[2].color.to_array()[i] as f32) / 255.0;
                        }
                        if let Some(t) = tex {
                            let mut u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                            let mut vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                            if t.repeat {
                                u = u.rem_euclid(1.0);
                                vv = vv.rem_euclid(1.0);
                            }
                            let tx = ((u * t.w as f32) as usize).min(t.w - 1);
                            let ty = ((vv * t.h as f32) as usize).min(t.h - 1);
                            let s = t.px[ty * t.w + tx];
                            for i in 0..4 {
                                c[i] *= s[i];
                            }
                        }
                        let d = &mut img[py * w + px];
                        for i in 0..3 {
                            d[i] = c[i] + d[i] * (1.0 - c[3]);
                        }
                    }
                }
            }
        }
        img
    }
    fn raw(i: usize, ev: Vec<Event>) -> RawInput {
        RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32))), time: Some(i as f64 * 0.04), events: ev, ..Default::default() }
    }

    fn drums(n: usize) -> Vec<f32> {
        let mut s = 77u32;
        (0..n).map(|i| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            let nz = (s >> 8) as f32 / 8388608.0 - 1.0;
            let t = (i % 24000) as f32 / 48000.0;
            let t2 = ((i + 12000) % 24000) as f32 / 48000.0;
            let kick = (-t * 12.0).exp() * (std::f32::consts::TAU * (50.0 + 80.0 * (-t * 30.0).exp()) * t).sin();
            let snare = if (i / 24000) % 2 == 1 { (-t2 * 18.0).exp() * nz * 0.6 } else { 0.0 };
            0.5 * (kick + snare) + 0.02 * nz
        }).collect()
    }

    #[test]
    fn draws_and_clicks() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let p = DbCompParams::default();
        let sh = Shared::default();
        let mock = Mock;
        let setter = ParamSetter::new(&mock);
        let mut st = new_state("DbTest");
        let mut i = 0;
        for y in (50..430).step_by(17) {
            for x in (6..816).step_by(29) {
                let q = Pos2::new(x as f32, y as f32);
                let _ = ctx.run_ui(raw(i, vec![Event::PointerMoved(q), Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]), |c| frame(c, &setter, &mut st, &p, &sh));
                let _ = ctx.run_ui(raw(i + 1, vec![Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]), |c| frame(c, &setter, &mut st, &p, &sh));
                i += 2;
            }
        }
    }

    /// Pill text dead centre (rendered pixels).
    #[test]
    fn pill_text_is_centred() {
        let labels = ["ARC", "ELECTRO", "SAFE 0 dB", "MID"];
        for on in [false, true] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut texs = HashMap::new();
            let rects: Vec<Rect> = (0..labels.len()).map(|i| Rect::from_min_size(pos2(30.0 + i as f32 * 150.0, 100.0), vec2(96.0, 20.0))).collect();
            let mut last = None;
            for f in 0..3 {
                let out = ctx.run_ui(raw(f, vec![]), |c| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG)).show(c, |ui| {
                        for (i, r) in rects.iter().enumerate() {
                            let _ = pill(ui, *r, labels[i], on, ("t", i));
                        }
                    });
                });
                if f < 2 {
                    apply_delta(&mut texs, &out);
                }
                last = Some(out);
            }
            let img = raster(&ctx, last.unwrap(), &mut texs);
            let w = WIDTH as usize;
            for (i, r) in rects.iter().enumerate() {
                let bg = img[(r.center().y as usize) * w + (r.left() + 12.0) as usize];
                let inner = r.shrink2(vec2(10.0, 3.0));
                let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
                for y in inner.top() as usize..inner.bottom() as usize {
                    for x in inner.left() as usize..inner.right() as usize {
                        let q = img[y * w + x];
                        if (q[0] - bg[0]).abs() + (q[1] - bg[1]).abs() + (q[2] - bg[2]).abs() > 0.45 {
                            x0 = x0.min(x);
                            x1 = x1.max(x);
                            y0 = y0.min(y);
                            y1 = y1.max(y);
                        }
                    }
                }
                assert!(x0 < x1, "{}: no text", labels[i]);
                let (dx, dy) = ((x0 + x1 + 1) as f32 * 0.5 - r.center().x, (y0 + y1 + 1) as f32 * 0.5 - r.center().y);
                assert!(dx.abs() <= 1.0 && dy.abs() <= 1.0, "{} off centre ({dx:+.1}, {dy:+.1})", labels[i]);
            }
        }
    }

    /// SHOT_DIR: the real engine on a drum loop. 0 = snare-style, 1 = heavy + SAT.
    #[test]
    fn screenshots() {
        let Ok(dir) = std::env::var("SHOT_DIR") else { return };
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mock = Mock;
        let setter = ParamSetter::new(&mock);
        let mut texs = HashMap::new();
        for (n, (th, ra, at, re, ga, sat)) in [(-20.0f32, 4.0f32, 15.0f32, 90.0f32, 3.0f32, 0.3f32), (-30.0, 8.0, 3.0, 250.0, 6.0, 0.6)].iter().enumerate() {
            let p = DbCompParams::default();
            let stt = State { thresh: *th, ratio: *ra, attack_ms: *at, release_ms: *re, gain_db: *ga, sat: *sat, ..State::default() };
            let p = with_values(p, &stt);
            let sh = Arc::new(Shared::default());
            let mut e = Engine::new(48000.0).with_shared(sh.clone());
            let x = drums(48000 + 9000);
            let (mut l, mut r) = (x.clone(), x.clone());
            for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                e.process(a, b, &stt);
            }
            let mut st = new_state("DbShot");
            let mut last = None;
            for f in 0..3 {
                last = Some(ctx.run_ui(raw(n * 10 + f, vec![]), |c| frame(c, &setter, &mut st, &p, &sh)));
                if f < 2 {
                    apply_delta(&mut texs, last.as_ref().unwrap());
                }
            }
            render(&ctx, last.unwrap(), &mut texs, &format!("{}/{}{}.rgb", dir, NAME.to_lowercase(), n));
        }
    }

    /// Params whose values (and defaults, so the faders draw there) are the state's.
    fn with_values(mut p: DbCompParams, s: &State) -> DbCompParams {
        let f = |n: &str, v: f32, a: f32, b: f32, skew: Option<f32>| match skew {
            Some(k) => FloatParam::new(n, v, FloatRange::Skewed { min: a, max: b, factor: FloatRange::skew_factor(k) }),
            None => FloatParam::new(n, v, FloatRange::Linear { min: a, max: b }),
        };
        p.thresh = f("Threshold", s.thresh, -48.0, 0.0, None);
        p.ratio = f("Ratio", s.ratio, 1.0, 20.0, Some(-1.5));
        p.attack = f("Attack", s.attack_ms, 0.5, 200.0, Some(-2.0));
        p.release = f("Release", s.release_ms, 5.0, 2000.0, Some(-2.0));
        p.gain = f("Gain", s.gain_db, -18.0, 18.0, None);
        p.sat = f("Saturation", s.sat, 0.0, 1.0, None);
        p
    }
}
