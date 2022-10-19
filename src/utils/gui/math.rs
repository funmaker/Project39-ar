use egui::*;

use crate::math::{Color, from_euler, Isometry3, PI, Rot3, to_euler, Vec3};
use super::*;

impl SimpleInspect for Vec3 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.columns(3, |ui| {
			ui[0].add(DragValue::new(&mut self.x).speed(0.01).prefix("X: "));
			ui[1].add(DragValue::new(&mut self.y).speed(0.01).prefix("Y: "));
			ui[2].add(DragValue::new(&mut self.z).speed(0.01).prefix("Z: "));
		});
	}
}

impl SimpleInspect for Rot3 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.columns(3, |ui| {
			let (mut pitch, mut yaw, mut roll) = to_euler(*self);
			
			pitch *= 180.0 / PI;
			yaw *= 180.0 / PI;
			roll *= 180.0 / PI;
			
			let pch = ui[0].add(DragValue::new(&mut pitch).prefix("ψ: ").clamp_range(-90.0..=90.0)).changed();
			let ych = ui[1].add(DragValue::new(&mut yaw).prefix("θ: ")).changed();
			let rch = ui[2].add(DragValue::new(&mut roll).prefix("φ: ")).changed();
			
			if pch || ych || rch {
				*self = from_euler(pitch / 180.0 * PI, yaw / 180.0 * PI, roll / 180.0 * PI);
			}
		});
	}
}

impl SimpleInspect for Isometry3 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.vertical(|ui| {
			ui.inspect(&mut self.translation.vector, ());
			ui.inspect(&mut self.rotation, ());
		});
	}
}

impl SimpleInspect for Color {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.color_edit_button_rgba_unmultiplied(&mut self.data.0[0]);
	}
}

impl SimpleInspect for bool {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.checkbox(self, "");
	}
}
