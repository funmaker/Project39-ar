use egui::*;

use crate::math::{Color, from_euler, Isometry2, Isometry3, PI, Point2, Point3, Point4, Rot2, Rot3, to_euler, Vec2, Vec3, Vec4};
use super::*;

impl SimpleInspect for Vec2 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.columns(2, |ui| {
			ui[0].add(DragValue::new(&mut self.x).speed(0.01).prefix("X: "));
			ui[1].add(DragValue::new(&mut self.y).speed(0.01).prefix("Y: "));
		});
	}
}

impl SimpleInspect for Vec3 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.columns(3, |ui| {
			ui[0].add(DragValue::new(&mut self.x).speed(0.01).prefix("X: "));
			ui[1].add(DragValue::new(&mut self.y).speed(0.01).prefix("Y: "));
			ui[2].add(DragValue::new(&mut self.z).speed(0.01).prefix("Z: "));
		});
	}
}

impl SimpleInspect for Vec4 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.columns(4, |ui| {
			ui[0].add(DragValue::new(&mut self.x).speed(0.01).prefix("X: "));
			ui[1].add(DragValue::new(&mut self.y).speed(0.01).prefix("Y: "));
			ui[2].add(DragValue::new(&mut self.z).speed(0.01).prefix("Z: "));
			ui[3].add(DragValue::new(&mut self.z).speed(0.01).prefix("W: "));
		});
	}
}

impl SimpleInspect for Point2 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.inspect(&mut self.coords, ());
	}
}

impl SimpleInspect for Point3 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.inspect(&mut self.coords, ());
	}
}

impl SimpleInspect for Point4 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.inspect(&mut self.coords, ());
	}
}

impl SimpleInspect for Rot2 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		let mut ang = self.angle() * 180.0 / PI;
		
		if ui.add(DragValue::new(&mut ang).prefix("α: ")).changed() {
			*self = Rot2::new(ang / 180.0 * PI);
		}
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

impl SimpleInspect for Isometry2 {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		ui.vertical(|ui| {
			ui.inspect(&mut self.translation.vector, ());
			ui.inspect(&mut self.rotation, ());
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
