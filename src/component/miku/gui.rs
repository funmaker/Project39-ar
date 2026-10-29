use egui::{Grid, Id, RichText, ScrollArea, Ui};

use crate::application::Application;
use crate::math::Color;
use crate::utils::{end_row_interact, id_fmt};
use super::super::model::mmd::BodyPart;
use super::Miku;

pub fn list(ui: &mut Ui, id: impl Into<Id>, selected: Option<usize>, elements: impl Iterator<Item=(usize, RichText)>) -> Option<usize> {
	let mut new_selection = None;
	
	let id = id.into();
	
	ScrollArea::vertical()
		.id_salt(id)
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

pub fn ragdoll(ui: &mut Ui, selected: Option<usize>, miku: &Miku, application: &Application) -> Option<usize> {
	let mut new_selection = None;
	let width = ui.available_width().max(16.0);
	let height = (width * 4.0).clamp(128.0, 384.0);
	let (id, rect) = ui.allocate_space([width, height].into());
	let painter = ui.painter_at(rect);
	
	let mut part_id = 0;
	let mut part = |body_part: BodyPart, x1: f32, y1: f32, x2: f32, y2: f32| {
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
		
		let bone = miku.body_part(body_part, application)
		               .map(|rb| rb.bone);
		
		let response = ui.interact(rect, id.with(part_id), egui::Sense::click());
		let hover_opacity = if response.hovered() { 1.0 } else { 0.8 };
		let color = if bone.is_none() {
			Color::BLACK.opactiy(0.3)
		} else if bone == selected {
			Color::WHITE.opactiy(hover_opacity)
		} else {
			Color::BLACK.opactiy(hover_opacity)
		};
		
		if let Some(bone) = bone {
			if response.clicked() {
				new_selection = Some(bone);
			}
		}
		
		painter.rect_filled(rect, half_height * 0.0125, color);
	};
	
	part(BodyPart::Abdomen, -0.10, -0.40,  0.10, -0.10);
	part(BodyPart::Torso, -0.15, -0.60,  0.15, -0.45);
	part(BodyPart::Hip, -0.15, -0.05,  0.15,  0.10);
	
	part(BodyPart::Head, -0.10, -0.95,  0.10, -0.75);
	part(BodyPart::Neck, -0.05, -0.70,  0.05, -0.65);
	
	part(BodyPart::RightArm, -0.25, -0.60, -0.20, -0.25);
	part(BodyPart::RightForearm, -0.25, -0.20, -0.20,  0.15);
	part(BodyPart::RightHand, -0.25,  0.20, -0.20,  0.30);
	
	part(BodyPart::LeftArm,  0.20, -0.60,  0.25, -0.25);
	part(BodyPart::LeftForearm,  0.20, -0.20,  0.25,  0.15);
	part(BodyPart::LeftHand,  0.20,  0.20,  0.25,  0.30);
	
	part(BodyPart::RightThigh, -0.10,  0.15, -0.05,  0.40);
	part(BodyPart::RightCalf, -0.10,  0.45, -0.05,  0.80);
	part(BodyPart::RightFoot, -0.10,  0.85, -0.05,  0.95);
	
	part(BodyPart::LeftThigh,  0.05,  0.15,  0.10,  0.40);
	part(BodyPart::LeftCalf,  0.05,  0.45,  0.10,  0.80);
	part(BodyPart::LeftFoot,  0.05,  0.85,  0.10,  0.95);
	
	new_selection
}
