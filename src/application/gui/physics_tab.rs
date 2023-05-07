use egui::Ui;
use rapier3d::dynamics::ImpulseJointHandle;
use rapier3d::geometry::ColliderHandle;
use rapier3d::prelude::RigidBodyHandle;

use crate::application::{Application, Physics};


pub fn physics_ui(physics: &mut Physics, ui: &mut Ui, application: &Application) {
	use egui::*;
	use crate::utils::ExUi;
	
	let sel_rb = application.get_selection().rigid_body();
	let sel_col = application.get_selection().collider();
	let sel_joint = application.get_selection().joint();
	
	CollapsingHeader::new("Parameters")
		.default_open(true)
		.show(ui, |ui| {
			Grid::new("Parameters")
				.num_columns(2)
				.min_col_width(110.0)
				.show(ui, |ui| {
					ui.inspect_row("Gravity", &mut physics.gravity, ());
					ui.inspect_row("Time Scale", &mut physics.time_scale, (0.001, 0.0..=100.0));
					ui.inspect_row("Min CCD DT", &mut physics.integration_parameters.min_ccd_dt, (0.00001, 0.0..=1.0));
					ui.inspect_row("ERP", &mut physics.integration_parameters.erp, (0.001, 0.0..=1.0));
					ui.inspect_row("Damping", &mut physics.integration_parameters.damping_ratio, (0.001, 0.0..=1.0));
					ui.inspect_row("Joint ERP", &mut physics.integration_parameters.joint_erp, (0.001, 0.0..=1.0));
					ui.inspect_row("Joint Damping", &mut physics.integration_parameters.joint_damping_ratio, (0.001, 0.0..=1.0));
					ui.inspect_row("Max Linear Err", &mut physics.integration_parameters.allowed_linear_error, (0.000001, 0.0..=1.0));
					ui.inspect_row("Max Pen Correct", &mut physics.integration_parameters.max_penetration_correction, (0.001, 0.0..=10.0));
					ui.inspect_row("Predictive Dist", &mut physics.integration_parameters.prediction_distance, (0.001, 0.0..=10.0));
					ui.inspect_row("Max Vel Iter", &mut physics.integration_parameters.max_velocity_iterations, 0..=usize::MAX);
					ui.inspect_row("Max Frict Iter", &mut physics.integration_parameters.max_velocity_friction_iterations, 0..=usize::MAX);
					ui.inspect_row("Max Stab Iter", &mut physics.integration_parameters.max_stabilization_iterations, 0..=usize::MAX);
					ui.inspect_row("Interleave Resl", &mut physics.integration_parameters.interleave_restitution_and_friction_resolution, ());
					ui.inspect_row("Min Island Size", &mut physics.integration_parameters.min_island_size, 0..=usize::MAX);
					ui.inspect_row("Max CCD Substeps", &mut physics.integration_parameters.max_ccd_substeps, 0..=usize::MAX);
				});
		});
	
	CollapsingHeader::new(format!("Rigid Bodies ({})", physics.rigid_body_set.len()))
		.id_source("Rigid Bodies")
		.open((sel_rb != RigidBodyHandle::invalid()).then_some(true))
		.show(ui, |ui| {
			for (handle, rb) in physics.rigid_body_set.iter_mut() {
				ui.inspect_collapsing()
				  .show(ui, rb, (handle, application, &mut physics.collider_set, &mut physics.impulse_joint_set));
			}
		});
	
	CollapsingHeader::new(format!("Colliders ({})", physics.collider_set.len()))
		.id_source("Colliders")
		.open((sel_col != ColliderHandle::invalid()).then_some(true))
		.show(ui, |ui| {
			for (handle, col) in physics.collider_set.iter_mut() {
				ui.inspect_collapsing()
				  .show(ui, col, (handle, application));
			}
		});
	
	CollapsingHeader::new(format!("Joints ({})", physics.impulse_joint_set.len()))
		.id_source("Joints")
		.open((sel_joint != ImpulseJointHandle::invalid()).then_some(true))
		.show(ui, |ui| {
			for (handle, joint) in physics.impulse_joint_set.iter_mut() {
				ui.inspect_collapsing()
				  .show(ui, joint, (handle, application));
			}
		});
}
