use egui::{Context, Id, Theme, Window};

const OPEN: &str = "projectile-visual-debug-open";

pub fn open(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp(Id::new(OPEN), true));
}

pub fn show(ctx: &Context) {
    let mut open = ctx.data_mut(|data| data.get_temp::<bool>(Id::new(OPEN)).unwrap_or(false));
    if !open {
        return;
    }
    Window::new("egui Visual Debug")
        .open(&mut open)
        .default_width(640.0)
        .show(ctx, |ui| {
            ui.label("Built-in egui inspection and style tools");
            let mut debug_on_hover = ctx.debug_on_hover();
            if ui.checkbox(&mut debug_on_hover, "Show widget diagnostics on hover").changed() {
                ctx.set_debug_on_hover(debug_on_hover);
            }
            ui.separator();
            ui.collapsing("Inspection", |ui| ctx.inspection_ui(ui));
            ui.collapsing("Memory", |ui| ctx.memory_ui(ui));
            ui.collapsing("Settings", |ui| ctx.settings_ui(ui));
            ui.collapsing("Dark style", |ui| ctx.style_ui(ui, Theme::Dark));
            ui.collapsing("Light style", |ui| ctx.style_ui(ui, Theme::Light));
        });
    ctx.data_mut(|data| data.insert_temp(Id::new(OPEN), open));
}
