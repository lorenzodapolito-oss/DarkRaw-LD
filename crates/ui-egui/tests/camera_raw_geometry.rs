//! Camera Raw's Geometry controls, through the dialog's agent entry point.
use photocraft_ui_egui::{PhotocraftApp, camera_raw_ui};
use serde_json::{Value, json};

fn app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 32, "height": 32})).unwrap();
    app.run("layer.new.layer", json!({})).unwrap();
    app.run("edit.fill", json!({"color": "#808080"})).unwrap();
    app
}

fn ui(app: &mut PhotocraftApp, ctx: &egui::Context, ui: Value) -> Value {
    camera_raw_ui::menu(app, ctx, "filter.cameraRaw", &json!({"ui": ui})).unwrap().unwrap()
}

#[test]
fn geometry_settings_are_part_of_the_filter_and_one_undoable_edit() {
    let ctx = egui::Context::default();
    let mut app = app();
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({})).unwrap().unwrap();
    let set = ui(&mut app, &ctx, json!({"set": {"geoRotate": 3.5, "geoScale": 20, "geoVertical": -30}}));
    assert_eq!(set["params"]["geoRotate"], 3.5);
    assert_eq!(set["params"]["geoScale"], 20.0);
    assert_eq!(set["params"]["geoVertical"], -30.0);
    assert_eq!(set["params"]["geoDistortion"], 0.0);
    assert_eq!(ui(&mut app, &ctx, json!({"undo": true}))["params"]["geoRotate"], 0.0);
    ui(&mut app, &ctx, json!({"set": {"geoRotate": 2}}));
    let steps = app.session.active().unwrap().history.past_len();
    camera_raw_ui::menu(&mut app, &ctx, "filter.cameraRaw", &json!({"ui": {"commit": true}})).unwrap().unwrap();
    assert_eq!(app.session.active().unwrap().history.past_len(), steps + 1);
}
