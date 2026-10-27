//! Camera Raw's Auto button, through the dialog's agent entry point (`{"ui":{"auto":true}}`).
use photocraft_ui_egui::{PhotocraftApp, camera_raw_ui};
use serde_json::{Value, json};

/// A dark picture: a dim field with a slightly lighter block, so it has some spread.
fn dark_app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 32, "height": 32})).unwrap();
    app.run("layer.new.layer", json!({})).unwrap();
    app.run("edit.fill", json!({"color": "#141414"})).unwrap();
    app.run("select.rect", json!({"x": 0, "y": 0, "width": 16, "height": 32})).unwrap();
    app.run("edit.fill", json!({"color": "#4a4a4a"})).unwrap();
    app.run("select.deselect", json!({})).unwrap();
    app
}

fn ui(app: &mut PhotocraftApp, ctx: &egui::Context, ui: Value) -> Value {
    camera_raw_ui::menu(app, ctx, "filter.cameraRaw", &json!({"ui": ui})).unwrap().unwrap()
}

#[test]
fn auto_brightens_a_dark_picture_as_one_undoable_edit() {
    let ctx = egui::Context::default();
    let mut app = dark_app();
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    let before = ui(&mut app, &ctx, json!({}));
    assert_eq!(before["params"]["exposure"], 0.0);
    let after = ui(&mut app, &ctx, json!({"auto": true}));
    assert!(after["params"]["exposure"].as_f64().unwrap() > 0.2, "{}", after["params"]);
    assert_ne!(after["histogram"]["red"], before["histogram"]["red"], "the preview was re-rendered");
    // Other settings are untouched.
    assert_eq!(after["params"]["vibrance"], 0.0);
    // One Ctrl+Z inside the dialog returns to the opening settings.
    let undone = ui(&mut app, &ctx, json!({"undo": true}));
    assert_eq!(undone["params"]["exposure"], 0.0);
    // Auto again, then OK: the layer changes by one history step.
    ui(&mut app, &ctx, json!({"auto": true}));
    let steps = app.session.active().unwrap().history.past_len();
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({"ui": {"commit": true}})).unwrap().unwrap();
    assert_eq!(app.session.active().unwrap().history.past_len(), steps + 1);
}

#[test]
fn auto_on_a_flat_picture_changes_nothing_and_a_bad_value_is_an_error() {
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 16, "height": 16})).unwrap();
    app.run("layer.new.layer", json!({})).unwrap();
    app.run("edit.fill", json!({"color": "#808080"})).unwrap();
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    let after = ui(&mut app, &ctx, json!({"auto": true}));
    assert_eq!(after["params"]["exposure"], 0.0);
    assert_eq!(after["params"]["contrast"], 0.0);
    let bad = camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({"ui": {"auto": "yes"}}));
    assert!(matches!(bad, Some(Err(_))), "{bad:?}");
}

fn warm_app(color: &str) -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 16, "height": 16})).unwrap();
    app.run("layer.new.layer", json!({})).unwrap();
    app.run("edit.fill", json!({"color": color})).unwrap();
    app
}

#[test]
fn the_white_balance_pick_and_auto_white_balance_cool_a_warm_picture() {
    let ctx = egui::Context::default();
    let mut app = warm_app("#c8a070");
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    let picked = ui(&mut app, &ctx, json!({"whiteBalanceAt": [0.5, 0.5]}));
    let t = picked["params"]["temperature"].as_f64().unwrap();
    assert!(t < -10.0, "a warm pick is cooled, got {t}");
    // One undo returns to As Shot.
    assert_eq!(ui(&mut app, &ctx, json!({"undo": true}))["params"]["temperature"], 0.0);
    // Auto white balance reaches the same on a flat picture.
    let auto = ui(&mut app, &ctx, json!({"autoWhiteBalance": true}));
    assert_eq!(auto["params"]["temperature"].as_f64().unwrap(), t);
    assert_eq!(auto["params"]["exposure"], 0.0, "only Temperature and Tint move");
}

#[test]
fn a_white_balance_pick_with_nothing_to_balance_or_a_bad_point_is_an_error() {
    let ctx = egui::Context::default();
    let mut app = warm_app("#000000");
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    for bad in
        [json!({"whiteBalanceAt": [0.5, 0.5]}), json!({"whiteBalanceAt": [0.5]}), json!({"whiteBalanceAt": "centre"}), json!({"whiteBalanceAt": [0.5, "x"]})]
    {
        let r = camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({"ui": bad}));
        assert!(matches!(r, Some(Err(_))), "{bad}: {r:?}");
    }
    // Off-picture points clamp to the edge instead of panicking.
    let mut app = warm_app("#c8a070");
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    assert!(ui(&mut app, &ctx, json!({"whiteBalanceAt": [-5.0, 9.0]}))["params"]["temperature"].as_f64().unwrap() < 0.0);
}
