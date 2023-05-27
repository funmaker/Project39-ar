use std::hash::Hash;
use egui::*;

use crate::debug;
use crate::application::{Application, Entity, EntityRef};
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
				.min_col_width(0.0)
				.num_columns(4)
				.show(ui, |ui| {
					for entity in application.root_entities() {
						entity_tree(ui, entity, application, &sel_ent, 0, false);
					}
				});
		});
	
	ui.add_space(8.0);
	
	if let Some(entity) = sel_ent.get(application)
	                             .or_else(|| application.pov.get(application)) {
		ScrollArea::vertical()
			.id_source(entity.id)
			.show(ui, |ui| ui.inspect(entity, application));
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
			ui.min_rect().min.x + 16.0,
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

fn entity_tree(ui: &mut Ui, entity: &Entity, application: &Application, sel_ent: &EntityRef, level: usize, default_open: bool) {
	let id = Id::new(entity.id);
	let mut open = ui.ctx().data_mut(|d| d.get_persisted(id).unwrap_or(default_open));
	
	if entity.children().is_empty() {
		ui.label("");
	} else {
		let (icon_id, rect) = ui.allocate_space(vec2(14.0, 14.0));
		let (rect, _) = ui.spacing().icon_rectangles(rect);
		let response = ui.interact(rect, icon_id, Sense::click());
		collapsing_header::paint_default_icon(ui, if open { 1.0 } else { 0.0 }, &response);
		
		if response.clicked() {
			open = !open;
			ui.ctx().data_mut(|d| d.insert_persisted(id, open));
		}
	}
	
	let name = "|    ".to_string().repeat(level) + &entity.name;
	
	if entity == sel_ent {
		ui.label(RichText::new(format!("{:04}", entity.id)).monospace().strong());
		ui.label(RichText::new(name).strong());
	} else {
		ui.label(RichText::new(format!("{:04}", entity.id)).monospace());
		ui.label(RichText::new(name));
	}
	
	ui.allocate_space(ui.available_size());
	
	if end_row_interact(ui, entity.id).clicked() {
		application.select(entity);
	}
	
	if open {
		let single_child = entity.children().len() == 1;
		
		for child in entity.children().iter() {
			if let Some(child) = child.get(application) {
				entity_tree(ui, child, application, sel_ent, level + 1, single_child);
			} else {
				ui.label(RichText::new("NONE").monospace());
				ui.label(RichText::new("Broken Child").monospace().italics());
				ui.end_row();
			}
		}
	}
}
