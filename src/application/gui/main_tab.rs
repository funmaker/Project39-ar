use std::hash::Hash;
use std::ops::RangeInclusive;
use egui::*;
use crate::application::GuiSelection;
use crate::debug;
use crate::utils::{end_row_interact, ExUi, id_fmt};
use super::super::{Application, Entity, EntityRef};


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
				debug_flag_checkbox(&mut ui[1], "DebugHideUnselected", "Only Draw Selected");
			});

			debug_flag_slider(ui, "DebugGizmoSize", 0.0..=2.0, "Gizmo Size");
			debug_flag_slider(ui, "DebugGizmoFallOff", 0.0..=3.0, "Gizmo Size Fall Off");
		});
	
	ui.separator();
	ui.label(RichText::new("Scene").strong());
	
	let selections = application.gui_selections().clone();
	let selected_ent = selections.selection.entity(&application.physics.borrow_mut());
	let hovered_cmp = selections.hover.entity(&application.physics.borrow_mut());
	
	ScrollArea::vertical()
		.max_height(128.0)
		.show(ui, |ui| {
			Grid::new("Entity List")
				.striped(true)
				.min_col_width(0.0)
				.num_columns(4)
				.show(ui, |ui| {
					for entity in application.root_entities() {
						entity_tree(ui, entity, application, &selected_ent, &hovered_cmp, 0, false);
					}
				});
		});
	
	ui.add_space(8.0);
	
	let selected_ent = application.gui_selections().selection.entity(&application.physics.borrow_mut());
	if let Some(entity) = selected_ent.get(application)
	                                  .or_else(|| application.pov.get(application)) {
		ScrollArea::vertical()
			.id_salt(entity.id)
			.show(ui, |ui| ui.inspect(entity, application));
	}
}


fn debug_flag_checkbox(ui: &mut Ui, flag: &str, label: impl Into<WidgetText>) {
	let mut value = debug::get_flag_or_default(flag);
	if Checkbox::new(&mut value, label).ui(ui).changed() {
		debug::set_flag(flag, value);
	}
}

fn debug_flag_slider(ui: &mut Ui, flag: &str, range: RangeInclusive<f32>, label: impl Into<WidgetText>) {
	let mut value: f32 = debug::get_flag_or_default(flag);
	if Slider::new(&mut value, range).text(label).ui(ui).changed() {
		debug::set_flag(flag, value);
	}
}

fn entity_tree(ui: &mut Ui, entity: &Entity, application: &Application, selected_ent: &EntityRef, hovered_ent: &EntityRef, level: usize, default_open: bool) {
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
	
	let color = match (entity == selected_ent, entity == hovered_ent) {
		(true, true) => Color32::WHITE.gamma_multiply(0.8),
		(true, false) => Color32::WHITE,
		(false, true) => ui.style().noninteractive().fg_stroke.color.gamma_multiply(0.7),
		(false, false) => ui.style().noninteractive().fg_stroke.color,
	};
	
	ui.label(id_fmt(entity.id, "").color(color));
	ui.label(RichText::new(name).color(color));
	
	ui.allocate_space(ui.available_size());
	
	let response = end_row_interact(ui, entity.id);
	if response.clicked() {
		application.select(entity, true);
	}
	
	let hovered = ui.track_hover(&response);
	if response.hovered() && !hovered {
		application.gui_selections_mut().hover = entity.into();
	} else if !response.hovered() && hovered {
		application.gui_selections_mut().hover = GuiSelection::None;
	}
	
	if open {
		let single_child = entity.children().len() == 1;
		
		for child in entity.children().iter() {
			if let Some(child) = child.get(application) {
				entity_tree(ui, child, application, selected_ent, hovered_ent, level + 1, single_child);
			} else {
				ui.label(RichText::new("NONE").monospace());
				ui.label(RichText::new("Broken Child").monospace().italics());
				ui.end_row();
			}
		}
	}
}
