use bevy::app::Plugin;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, ui_example)
            .add_plugins(EguiPlugin::default());
    }
}

fn ui_example(mut contexts: EguiContexts) {
    egui::Window::new("Main Menu").show(contexts.ctx_mut().unwrap(), |ui| {
        ui.label("Hello, Rust + Bevy + Egui!");
        if ui.button("Connect to Server").clicked() {
            println!("TODO: implement connection");
        }
        if ui.button("Quit").clicked() {
            std::process::exit(0);
        }
    });
}
