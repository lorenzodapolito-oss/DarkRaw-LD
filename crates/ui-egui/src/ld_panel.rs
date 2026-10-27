//! LD Panel: a docked panel beside the photo, before the normal right-hand dock, with one-click
//! recipes for landscape and night photography, in six sections (Main, Color Correction, Night
//! Time, Editing, Final Touch, Export). Opened from the LD Panel menu. Every button runs ordinary
//! engine commands (selections, adjustment layers, filters, stacking, export), so the control
//! channel and the Actions panel see exactly what the buttons do.

use egui::RichText;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::theme::Tokens;

const PREFIX: &str = "ldpanel.";
const SECTIONS: [&str; 6] = ["main", "color", "night", "editing", "final", "export"];
const WIDTH: f32 = 330.0;

/// The panel's view state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LdPanel {
    pub open: bool,
    /// One of `SECTIONS`.
    #[serde(default = "main_section")]
    pub section: String,
    /// Luminosity mask level, 1..=3 (higher is narrower).
    #[serde(default = "one")]
    pub level: u32,
    /// Selective colour: saturation / hue / lightness amounts for reds, yellows, greens, cyans,
    /// blues and magentas (-100..100), and which of the three the sliders show.
    #[serde(default)]
    pub hsl: Vec<f32>,
    #[serde(default)]
    pub hsl_mode: u8,
    /// Dodge / Burn brush sliders: size (px), exposure (%), hardness (%).
    #[serde(default = "brush_default")]
    pub brush: [f32; 3],
}

fn brush_default() -> [f32; 3] {
    [200.0, 30.0, 0.0]
}

fn main_section() -> String {
    "main".into()
}

fn one() -> u32 {
    1
}

impl Default for LdPanel {
    fn default() -> Self {
        LdPanel { open: false, section: main_section(), level: 1, hsl: vec![0.0; 18], hsl_mode: 0, brush: brush_default() }
    }
}

/// Menu checkmark for LD Panel › section.
pub fn checked(app: &PhotocraftApp, id: &str) -> Option<bool> {
    let name = id.strip_prefix(PREFIX)?;
    SECTIONS.contains(&name).then(|| app.ui.ldpanel.open && app.ui.ldpanel.section == name)
}

/// Ids this module owns (for parity / enablement).
pub fn handles(id: &str) -> bool {
    id.strip_prefix(PREFIX).is_some_and(|n| n == "hide" || SECTIONS.contains(&n))
}

/// A menu command: show the panel on a section, or hide it.
pub fn menu(app: &mut PhotocraftApp, id: &str, _params: &Value) -> Option<Result<Value, String>> {
    let name = id.strip_prefix(PREFIX)?;
    if name == "hide" {
        app.ui.ldpanel.open = false;
    } else if SECTIONS.contains(&name) {
        app.ui.ldpanel.open = true;
        app.ui.ldpanel.section = name.to_string();
    } else {
        return None;
    }
    Some(Ok(json!({ "open": app.ui.ldpanel.open, "section": app.ui.ldpanel.section })))
}

fn section_label(name: &str) -> &'static str {
    match name {
        "color" => crate::i18n::t("Color Correction"),
        "night" => crate::i18n::t("Night Time"),
        "editing" => crate::i18n::t("Editing"),
        "final" => crate::i18n::t("Final Touch"),
        "export" => crate::i18n::t("Export"),
        _ => crate::i18n::t("Main"),
    }
}

/// The mask command a Lights / Midtones / Darks button sends.
pub(crate) fn mask_params(level: u32, kind: &str, operation: &str) -> Value {
    let operation = if matches!(operation, "add" | "subtract") { operation } else { "new" };
    json!({ "kind": kind, "level": level.clamp(1, 3), "operation": operation })
}

/// A hue/saturation adjustment from the six slider triples (reds … magentas); only the ranges
/// that were moved are sent.
pub(crate) fn hsl_params(values: &[f32]) -> Value {
    let names = ["reds", "yellows", "greens", "cyans", "blues", "magentas"];
    let mut out = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        let at = |k: usize| values.get(k * 6 + i).copied().filter(|v| v.is_finite()).unwrap_or(0.0).clamp(-100.0, 100.0);
        let (s, h, l) = (at(0), at(1), at(2));
        if s != 0.0 || h != 0.0 || l != 0.0 {
            out.insert((*name).into(), json!({"saturation": s, "hue": h * 1.8, "lightness": l}));
        }
    }
    Value::Object(out)
}

/// Something a button does to the app's own state rather than through one engine command.
enum Act {
    /// Pick the Dodge or Burn tool with a tonal range.
    Tone { burn: bool, range: &'static str },
    /// Push the panel's brush sliders to the brush.
    Brush,
}

/// Run commands in order, stopping at the first that fails (its message goes to the status bar).
fn run_all(app: &mut PhotocraftApp, steps: &[(&str, Value)]) {
    for (cmd, params) in steps {
        if let Err(e) = app.run(cmd, params.clone()) {
            app.ui.status = e.to_string();
            return;
        }
    }
}

fn button(ui: &mut egui::Ui, label: &str) -> bool {
    crate::widgets::secondary_button(ui, label, ui.available_width()).clicked()
}

fn small(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    crate::widgets::secondary_button(ui, label, width).clicked()
}

fn group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.field_border)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(12.0).color(t.text));
        ui.add_space(4.0);
        body(ui);
    });
    ui.add_space(6.0);
}

/// Draw the panel (a right-hand side panel placed before the dock, so beside the photo).
pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if !app.ui.ldpanel.open {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let mut state = app.ui.ldpanel.clone();
    if state.hsl.len() != 18 {
        state.hsl = vec![0.0; 18];
    }
    let mut steps: Vec<(&str, Value)> = Vec::new();
    let mut acts: Vec<Act> = Vec::new();
    let mut close = false;
    let has_doc = app.session.active().is_some();
    crate::direction::end_panel("ld-panel")
        .resizable(false)
        .exact_size(WIDTH)
        .frame(egui::Frame::NONE.fill(t.card).inner_margin(8.0))
        .show(ui, |ui| {
            // smaller buttons and tighter spacing: the panel keeps its size, the content fits it
            {
                let sp = ui.spacing_mut();
                sp.button_padding = egui::vec2(4.0, 1.0);
                sp.item_spacing = egui::vec2(4.0, 3.0);
                sp.interact_size.y = 20.0;
                sp.slider_width = 70.0;
                ui.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("LD Panel").strong().size(14.0).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::button(ui, "x", 20.0, false, crate::i18n::t("Close")).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);
            egui::Grid::new("ld-sections").num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, name) in SECTIONS.iter().enumerate() {
                    let on = state.section == *name;
                    let r = ui.add_sized([(WIDTH - 16.0 - 8.0) / 3.0, 20.0], egui::Button::new(RichText::new(section_label(name)).size(10.0)).selected(on));
                    if r.clicked() {
                        state.section = (*name).to_string();
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(6.0);
            if !has_doc {
                ui.weak(crate::i18n::t("Open a document to use the panel."));
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // leave room for the scroll bar so nothing runs under the photo
                ui.set_max_width(ui.available_width() - 12.0);
                match state.section.as_str() {
                "color" => color_section(ui, &mut state, &mut steps),
                "night" => night_section(ui, &mut steps),
                "editing" => editing_section(ui, &mut steps),
                "final" => final_section(ui, &mut steps),
                "export" => export_section(ui, &mut steps),
                _ => main_section_ui(ui, &mut state, &mut steps, &mut acts),
                }
            });
        });
    app.ui.ldpanel = state;
    if close {
        app.ui.ldpanel.open = false;
    }
    for act in acts {
        match act {
            Act::Tone { burn, range } => {
                app.ui.tool_options.tone_range = range.to_string();
                let tool = if burn { "tools.select.burn" } else { "tools.select.dodge" };
                if let Err(e) = app.run(tool, json!({})) {
                    app.ui.status = e.to_string();
                }
            }
            Act::Brush => {
                let [size, exposure, hardness] = app.ui.ldpanel.brush;
                app.ui.tool_options.exposure = exposure.clamp(1.0, 100.0);
                if let Err(e) = app.run("tools.setBrush", json!({"size": size.max(1.0), "hardness": hardness.clamp(0.0, 100.0) / 100.0})) {
                    app.ui.status = e.to_string();
                }
            }
        }
    }
    run_all(app, &steps);
}

fn main_section_ui(ui: &mut egui::Ui, state: &mut LdPanel, steps: &mut Vec<(&'static str, Value)>, acts: &mut Vec<Act>) {
    group(ui, crate::i18n::t("Luminosity Masks"), |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("Level"));
            for n in 1..=3u32 {
                ui.selectable_value(&mut state.level, n, n.to_string());
            }
        });
        // three columns, as the tones run from dark to light, each with its + and − below
        let col = ((ui.available_width() - 12.0) / 3.0).floor();
        ui.horizontal(|ui| {
            for (kind, label) in [("darks", crate::i18n::t("Shadows")), ("mids", crate::i18n::t("Midtones")), ("lights", crate::i18n::t("Lights"))] {
                ui.vertical(|ui| {
                    ui.set_width(col);
                    if small(ui, label, col) {
                        steps.push(("select.luminosityMask", mask_params(state.level, kind, "new")));
                    }
                    ui.horizontal(|ui| {
                        if small(ui, "+", (col - 4.0) / 2.0) {
                            steps.push(("select.luminosityMask", mask_params(state.level, kind, "add")));
                        }
                        if small(ui, "−", (col - 4.0) / 2.0) {
                            steps.push(("select.luminosityMask", mask_params(state.level, kind, "subtract")));
                        }
                    });
                });
            }
        });
        ui.add_space(2.0);
        if button(ui, crate::i18n::t("Selection to layer mask")) {
            steps.push(("layer.layerMask.revealSelection", json!({})));
        }
    });
    group(ui, crate::i18n::t("Dodge and Burn"), |ui| {
        let col = ((ui.available_width() - 12.0) / 3.0).floor();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(col);
                ui.label(RichText::new(crate::i18n::t("Dodge")).strong());
                for (range, label) in [("highlights", crate::i18n::t("Lights")), ("midtones", crate::i18n::t("Midtones")), ("shadows", crate::i18n::t("Shadows"))] {
                    if small(ui, label, col) {
                        acts.push(Act::Tone { burn: false, range });
                    }
                }
            });
            ui.vertical(|ui| {
                ui.set_width(col);
                ui.label(RichText::new(crate::i18n::t("Brush")).strong());
                ui.spacing_mut().slider_width = (col - 44.0).max(24.0);
                for (i, (label, max)) in [(crate::i18n::t("Size"), 2000.0f32), (crate::i18n::t("Opacity"), 100.0), (crate::i18n::t("Hardness"), 100.0)].into_iter().enumerate() {
                    ui.label(label);
                    if ui.add(egui::Slider::new(&mut state.brush[i], 0.0..=max).show_value(true).trailing_fill(false)).changed() {
                        acts.push(Act::Brush);
                    }
                }
            });
            ui.vertical(|ui| {
                ui.set_width(col);
                ui.label(RichText::new(crate::i18n::t("Burn")).strong());
                for (range, label) in [("highlights", crate::i18n::t("Lights")), ("midtones", crate::i18n::t("Midtones")), ("shadows", crate::i18n::t("Shadows"))] {
                    if small(ui, label, col) {
                        acts.push(Act::Tone { burn: true, range });
                    }
                }
            });
        });
    });
    group(ui, crate::i18n::t("Soft Glow (Orton)"), |ui| {
        for (night, title) in [(false, crate::i18n::t("Day")), (true, crate::i18n::t("Night"))] {
            ui.horizontal(|ui| {
                ui.label(title);
                for px in [1.5f64, 3.0, 5.0, 10.0] {
                    if small(ui, &format!("{px}px"), 38.0) {
                        let (blend, opacity) = if night { ("Soft Light", 0.8) } else { ("Screen", 0.35) };
                        steps.push(("layer.duplicate", json!({})));
                        steps.push(("filter.blur.gaussianBlur", json!({"radius": px})));
                        steps.push(("layer.setProps", json!({"blend": blend, "opacity": opacity, "name": format!("Glow {px}px")})));
                    }
                }
            });
        }
    });
    group(ui, crate::i18n::t("Extra Lights Enhance"), |ui| {
        ui.horizontal(|ui| {
            for (pct, opacity) in [(50, 0.5f64), (100, 1.0)] {
                if small(ui, &format!("+{pct}%"), 70.0) {
                    steps.push(("select.luminosityMask", mask_params(2, "lights", "new")));
                    steps.push(("layer.duplicate", json!({})));
                    steps.push(("layer.setProps", json!({"blend": "Screen", "opacity": opacity, "name": format!("Lights +{pct}%")})));
                    steps.push(("layer.layerMask.revealSelection", json!({})));
                }
            }
        });
    });
    group(ui, crate::i18n::t("Exposure Compensation (EV)"), |ui| {
        ui.horizontal_wrapped(|ui| {
            for ev in [-1.0f64, -0.5, -0.2, 0.2, 0.5, 1.0] {
                if small(ui, &format!("{ev:+}"), 34.0) {
                    steps.push(("layer.newAdjustmentLayer.exposure", json!({"exposure": ev})));
                }
            }
        });
    });
}

fn color_section(ui: &mut egui::Ui, state: &mut LdPanel, steps: &mut Vec<(&'static str, Value)>) {
    group(ui, crate::i18n::t("Selective Color Correction"), |ui| {
        ui.horizontal(|ui| {
            for (i, l) in [crate::i18n::t("Saturation"), crate::i18n::t("Hue"), crate::i18n::t("Lightness")].into_iter().enumerate() {
                ui.selectable_value(&mut state.hsl_mode, i as u8, l);
            }
        });
        let mode = usize::from(state.hsl_mode.min(2));
        for (i, name) in [crate::i18n::t("Red"), crate::i18n::t("Yellow"), crate::i18n::t("Green"), crate::i18n::t("Cyan"), crate::i18n::t("Blue"), crate::i18n::t("Magenta")].into_iter().enumerate() {
            if let Some(v) = state.hsl.get_mut(mode * 6 + i) {
                ui.horizontal(|ui| {
                    ui.add_sized([56.0, 18.0], egui::Label::new(name));
                    ui.add(egui::Slider::new(v, -100.0..=100.0).show_value(true));
                });
            }
        }
        ui.horizontal(|ui| {
            if small(ui, crate::i18n::t("Apply"), 80.0) {
                steps.push(("layer.newAdjustmentLayer.hueSaturation", hsl_params(&state.hsl)));
            }
            if small(ui, crate::i18n::t("Reset"), 80.0) {
                state.hsl = vec![0.0; 18];
            }
        });
    });
    group(ui, crate::i18n::t("Color Cast and Saturation"), |ui| {
        if button(ui, crate::i18n::t("Warm photo filter")) {
            steps.push(("layer.newAdjustmentLayer.photoFilter", json!({"filter": "warming85", "density": 25})));
        }
        if button(ui, crate::i18n::t("Cool photo filter")) {
            steps.push(("layer.newAdjustmentLayer.photoFilter", json!({"filter": "cooling80", "density": 25})));
        }
        if button(ui, crate::i18n::t("Reduce saturation")) {
            steps.push(("layer.newAdjustmentLayer.hueSaturation", json!({"saturation": -15})));
        }
        if button(ui, crate::i18n::t("Reduce blue (blue hour)")) {
            steps.push(("layer.newAdjustmentLayer.hueSaturation", json!({"blues": {"saturation": -25}})));
        }
        if button(ui, crate::i18n::t("Reduce yellow cast")) {
            steps.push(("layer.newAdjustmentLayer.hueSaturation", json!({"yellows": {"saturation": -25}})));
        }
    });
}

fn night_section(ui: &mut egui::Ui, steps: &mut Vec<(&'static str, Value)>) {
    group(ui, crate::i18n::t("Highlights Recovery"), |ui| {
        if button(ui, crate::i18n::t("Recovery 50%")) {
            steps.push(("layer.newAdjustmentLayer.curves", json!({"points": [[0, 0], [192, 176], [255, 255]]})));
        }
        if button(ui, crate::i18n::t("Recovery 100%")) {
            steps.push(("layer.newAdjustmentLayer.curves", json!({"points": [[0, 0], [192, 160], [255, 255]]})));
        }
    });
    group(ui, crate::i18n::t("Exposure Stacking"), |ui| {
        if button(ui, crate::i18n::t("Step 1: align photos")) {
            steps.push(("edit.autoAlignLayers", json!({"projection": "reposition"})));
        }
        if button(ui, crate::i18n::t("Step 2: blend layers")) {
            steps.push(("edit.autoBlendLayers", json!({"method": "stack"})));
        }
        if button(ui, crate::i18n::t("Merge layers")) {
            steps.push(("layer.mergeVisible", json!({})));
        }
    });
    group(ui, crate::i18n::t("Star Trails"), |ui| {
        if button(ui, crate::i18n::t("Set layers to Lighten")) {
            steps.push(("layer.setProps", json!({"blend": "Lighten"})));
        }
    });
}

fn editing_section(ui: &mut egui::Ui, steps: &mut Vec<(&'static str, Value)>) {
    group(ui, crate::i18n::t("Tone Filters"), |ui| {
        if button(ui, crate::i18n::t("Polarizer effect")) {
            steps.push(("layer.newAdjustmentLayer.hueSaturation", json!({"blues": {"saturation": 25, "lightness": -15}, "cyans": {"saturation": 15, "lightness": -15}})));
        }
        if button(ui, crate::i18n::t("Warm tonality filter")) {
            steps.push(("layer.newAdjustmentLayer.photoFilter", json!({"filter": "warming81", "density": 20})));
        }
        if button(ui, crate::i18n::t("Black and white")) {
            steps.push(("layer.newAdjustmentLayer.blackWhite", json!({})));
        }
    });
    group(ui, crate::i18n::t("Selective Desaturation"), |ui| {
        ui.horizontal_wrapped(|ui| {
            for px in [50.0f64, 100.0, 200.0, 300.0] {
                if small(ui, &format!("{px}px"), 50.0) {
                    steps.push(("layer.newAdjustmentLayer.hueSaturation", json!({"saturation": -40})));
                    steps.push(("layer.layerMask.revealSelection", json!({})));
                }
            }
        });
    });
    group(ui, crate::i18n::t("Focus Stacking"), |ui| {
        if button(ui, crate::i18n::t("Align layers")) {
            steps.push(("edit.autoAlignLayers", json!({"projection": "auto"})));
        }
        if button(ui, crate::i18n::t("Blend layers (stack)")) {
            steps.push(("edit.autoBlendLayers", json!({"method": "stack"})));
        }
    });
}

fn final_section(ui: &mut egui::Ui, steps: &mut Vec<(&'static str, Value)>) {
    group(ui, crate::i18n::t("Contrast Enhance"), |ui| {
        if button(ui, crate::i18n::t("Contrast +")) {
            steps.push(("layer.newAdjustmentLayer.brightnessContrast", json!({"contrast": 20})));
        }
        if button(ui, crate::i18n::t("Whites +")) {
            steps.push(("layer.newAdjustmentLayer.levels", json!({"inputWhite": 245})));
        }
    });
    group(ui, crate::i18n::t("Final Sharpening"), |ui| {
        for (label, amount) in [(crate::i18n::t("Strong (100%)"), 100), (crate::i18n::t("Medium (80%)"), 80), (crate::i18n::t("Light (50%)"), 50)] {
            if button(ui, label) {
                steps.push(("filter.sharpen.unsharpMask", json!({"amount": amount, "radius": 1.0, "threshold": 0})));
            }
        }
    });
}

fn export_section(ui: &mut egui::Ui, steps: &mut Vec<(&'static str, Value)>) {
    group(ui, crate::i18n::t("Utilities"), |ui| {
        if button(ui, crate::i18n::t("Convert to sRGB")) {
            steps.push(("edit.convertToProfile", json!({"profile": "srgb"})));
        }
        if button(ui, crate::i18n::t("Flatten image")) {
            steps.push(("layer.flattenImage", json!({})));
        }
    });
    group(ui, crate::i18n::t("Web Resize"), |ui| {
        for (label, width) in [(crate::i18n::t("Web 2048 px"), 2048), (crate::i18n::t("Web 1600 px"), 1600), (crate::i18n::t("Web 1080 px"), 1080)] {
            if button(ui, label) {
                steps.push(("image.duplicate", json!({})));
                steps.push(("image.imageSize", json!({"width": width})));
            }
        }
    });
    group(ui, crate::i18n::t("Quick Export"), |ui| {
        if button(ui, crate::i18n::t("Export as PNG")) {
            steps.push(("file.export.quickExportAsPng", json!({})));
        }
        if button(ui, crate::i18n::t("Quick Export")) {
            steps.push(("file.export.quickExport", json!({})));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_buttons_send_valid_params_whatever_the_state() {
        assert_eq!(mask_params(1, "lights", "new"), json!({"kind": "lights", "level": 1, "operation": "new"}));
        assert_eq!(mask_params(9, "darks", "junk"), json!({"kind": "darks", "level": 3, "operation": "new"}));
        assert_eq!(mask_params(0, "mids", "subtract"), json!({"kind": "mids", "level": 1, "operation": "subtract"}));
    }

    #[test]
    fn hsl_sends_only_the_moved_ranges_and_survives_bad_input() {
        let mut v = vec![0.0f32; 18];
        assert_eq!(hsl_params(&v), json!({}));
        v[2] = -40.0; // greens saturation
        v[6 + 4] = 50.0; // blues hue
        let p = hsl_params(&v);
        assert_eq!(p["greens"]["saturation"], -40.0);
        assert_eq!(p["blues"]["hue"], 90.0);
        assert!(p.get("reds").is_none());
        let odd = hsl_params(&[f32::NAN, 500.0]);
        assert!(odd.get("reds").is_none() && odd["yellows"]["saturation"] == 100.0);
    }

    #[test]
    fn the_menu_owns_the_sections() {
        for s in SECTIONS {
            assert!(handles(&format!("ldpanel.{s}")));
        }
        assert!(handles("ldpanel.hide") && !handles("ldpanel.other") && !handles("window.panel.timeline"));
    }
}
