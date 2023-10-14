use egui::{Grid, Id, RichText, ScrollArea, Ui};

use crate::application::Application;
use crate::component::ComponentBase;
use crate::math::Color;
use crate::utils::{end_row_interact, id_fmt};
use super::Miku;


pub fn miku_gui(miku: &Miku, ui: &mut Ui, application: &Application) {
	let model = match miku.model.get(application) {
		Some(model) => model,
		None => return,
	};
	
	let id = Id::new(&miku.entity(application).name).with("Miku Gui");
	let selected_bone = miku.gui_selection.get();
	
	let new_selection = ui.columns(3, |ui| {
		[
			list(&mut ui[0],
			     id.with("bones"),
			     selected_bone,
			     model.state.borrow()
			          .bones
			          .iter()
			          .map(|bone| bone.name.as_str().into())
			          .enumerate()),
			ragdoll(&mut ui[1], selected_bone),
			list(&mut ui[2],
			     id.with("rigidbodies"),
			     selected_bone,
			     model.state.borrow()
			          .rigid_bodies
			          .iter()
			          .map(|rb| rb
			                      .get(application)
			                      .map(|rb| (rb.bone, rb.entity(application).name.as_str().into()))
			                      .unwrap_or_else(|| (0, RichText::new("NULL").monospace().italics())))
			),
		].iter() // TODO: into_iter in new rust edition
		 .find_map(|&x| x)
	});
	
	if let Some(new_selection) = new_selection {
		miku.gui_selection.set(Some(new_selection));
	}
	
	ui.label("End");
}

fn list(ui: &mut Ui, id: impl Into<Id>, selected: Option<usize>, elements: impl Iterator<Item=(usize, RichText)>) -> Option<usize> {
	let mut new_selection = None;
	
	let id = id.into();
	
	ScrollArea::vertical()
		.id_source(id)
		.max_height(384.0)
		.show(ui, |ui| {
			Grid::new(id)
				.striped(true)
				.min_col_width(0.0)
				.num_columns(3)
				.show(ui, |ui| {
					for (row_id, (bone_id, label)) in elements.enumerate() {
						if Some(bone_id) == selected {
							ui.label(id_fmt(row_id, "").strong());
							ui.label(label.strong());
						} else {
							ui.label(id_fmt(row_id, ""));
							ui.label(label);
						}
						ui.allocate_space(ui.available_size());
						
						if end_row_interact(ui, id.with(row_id)).clicked() {
							new_selection = Some(bone_id);
						}
					}
					ui.allocate_space(ui.available_size());
				});
		});
	
	new_selection
}

fn ragdoll(ui: &mut Ui, selected: Option<usize>) -> Option<usize> {
	let mut new_selection = None;
	let width = ui.available_width().max(16.0);
	let height = (width * 4.0).clamp(128.0, 384.0);
	let (id, rect) = ui.allocate_space([width, height].into());
	let painter = ui.painter_at(rect);
	
	let mut part_id = 0;
	let mut part = |bone: usize, x1: f32, y1: f32, x2: f32, y2: f32| {
		part_id += 1;
		
		let half_width = rect.width() / 2.0;
		let half_height = rect.height() / 2.0;
		
		let rect = egui::Rect::from_min_max(
			[
				rect.min.x + x1 * half_height + half_width,
				rect.min.y + y1 * half_height + half_height,
			].into(),
			[
				rect.min.x + x2 * half_height + half_width,
				rect.min.y + y2 * half_height + half_height,
			].into(),
		);
		
		let response = ui.interact(rect, id.with(part_id), egui::Sense::click());
		let color = if Some(bone) == selected && response.hovered() {
			Color::WHITE
		} else if Some(bone) == selected || response.hovered() {
			Color::D_WHITE
		} else {
			Color::BLACK
		};
		
		if response.clicked() {
			new_selection = Some(bone);
		}
		
		painter.rect_filled(rect, half_height * 0.0125, color);
	};
	
	part(0, -0.10, -0.40,  0.10, -0.10);
	part(1, -0.15, -0.60,  0.15, -0.45);
	part(2, -0.15, -0.05,  0.15,  0.10);
	
	part(3, -0.10, -0.95,  0.10, -0.75);
	part(4, -0.05, -0.70,  0.05, -0.65);
	
	part(5, -0.25, -0.60, -0.20, -0.25);
	part(6, -0.25, -0.20, -0.20,  0.15);
	part(7, -0.25,  0.20, -0.20,  0.30);
	
	part(8,  0.20, -0.60,  0.25, -0.25);
	part(9,  0.20, -0.20,  0.25,  0.15);
	part(10,  0.20,  0.20,  0.25,  0.30);
	
	part(11, -0.10,  0.15, -0.05,  0.40);
	part(12, -0.10,  0.45, -0.05,  0.80);
	part(13, -0.10,  0.85, -0.05,  0.95);
	
	part(14,  0.05,  0.15,  0.10,  0.40);
	part(15,  0.05,  0.45,  0.10,  0.80);
	part(16,  0.05,  0.85,  0.10,  0.95);
	
	new_selection
}
