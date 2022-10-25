use std::hash::Hash;
use egui::*;

use crate::debug;
use crate::application::Application;
use crate::utils::ExUi;

pub fn main_ui(ui: &mut Ui, application: &Application) {
	CollapsingHeader::new("Debug Flags")
		.default_open(true)
		.show(ui, |ui| {
			ui.columns(2, |ui| {
				debug_flag_checkbox(&mut ui[0], "DebugEntityDraw", "Draw Entities");
				debug_flag_checkbox(&mut ui[0], "DebugBonesDraw", "Draw Bones");
				debug_flag_checkbox(&mut ui[0], "DebugCollidersDraw", "Draw Colliders");
				debug_flag_checkbox(&mut ui[1], "DebugJointsDraw", "Draw Joints");
				debug_flag_checkbox(&mut ui[1], "DebugRigidBodiesDraw", "Draw Rigid Bodies");
			});
		});
	
	ui.separator();
	ui.label(RichText::new("Scene").strong());
	
	let sel_ent = application.gui_selection.borrow().entity_or_component();
	
	ScrollArea::vertical()
		.max_height(128.0)
		.show(ui, |ui| {
			Grid::new("Entity List")
				.striped(true)
				.num_columns(3)
				.show(ui, |ui| {
					for (&eid, entity) in application.entities.iter() {
						if entity == sel_ent {
							ui.label(RichText::new(format!("{:04}", eid)).monospace().strong());
							ui.label(RichText::new(&entity.name).strong());
						} else {
							ui.label(RichText::new(format!("{:04}", eid)).monospace());
							ui.label(RichText::new(&entity.name));
						}
						
						ui.allocate_space(ui.available_size());
						
						if end_row_interact(ui, eid).clicked() {
							application.select(entity);
						}
					}
				});
		});
	
	ui.add_space(8.0);
	
	if let Some(entity) = sel_ent.get(application)
	                             .or_else(|| application.pov.get(application)) {
		ui.inspect(entity, application);
	}
}


fn debug_flag_checkbox(ui: &mut Ui, flag: &str, label: impl Into<WidgetText>) {
	let mut value = debug::get_flag_or_default(flag);
	if Checkbox::new(&mut value, label).ui(ui).changed() {
		debug::set_flag(flag, value);
	}
}

// Hack but will do until egui supports hovering over grid
fn end_row_interact(ui: &mut Ui, id: impl Hash) -> Response {
	let rect = Rect::from_min_max(
		[
			ui.min_rect().min.x,
			ui.cursor().min.y,
		].into(),
		[
			ui.cursor().min.x,
			ui.min_rect().max.y,
		].into()
	);
	
	ui.end_row();
	
	ui.interact(rect, Id::new(id), Sense::click())
}
