use egui::Ui;
use rapier3d::dynamics::{FrictionModel, ImpulseJointHandle, IntegrationParameters, SpringCoefficients};
use rapier3d::geometry::ColliderHandle;
use rapier3d::prelude::{PhysicsWorld, RigidBodyHandle};
use crate::application::GuiSelection;
use crate::utils::{id_fmt, ExUi, SelectionLevel};
use super::super::Application;


pub fn physics_ui(physics: &mut PhysicsWorld, ui: &mut Ui, application: &Application) {
	use egui::*;
	use crate::utils::ExUi;
	
	let selection = application.gui_selections().selection.clone();
	
	CollapsingHeader::new("Parameters")
		.default_open(true)
		.show(ui, |ui| {
			Grid::new("Parameters")
				.num_columns(2)
				.min_col_width(110.0)
				.show(ui, |ui| {
					ui.inspect_row("Gravity", &mut physics.gravity, ());
					ui.inspect_row("Min CCD DT", &mut physics.integration_parameters.min_ccd_dt, (0.00001, 0.0..=1.0));
					ui.inspect_row("Contact Soft", &mut physics.integration_parameters.contact_softness, ());
					ui.inspect_row("Static contact Soft", &mut physics.integration_parameters.static_contact_softness, ());
					ui.inspect_row("Warmstart coeff", &mut physics.integration_parameters.warmstart_coefficient, (0.01, 0.0..=100.0));
					ui.inspect_row("Warmstart joints", &mut physics.integration_parameters.warmstart_joints, ());
					ui.inspect_row("Length Unit", &mut physics.integration_parameters.length_unit, (0.0001, 0.0..=1000.0));
					ui.inspect_row("Norm allowed lin err", &mut physics.integration_parameters.normalized_allowed_linear_error, (0.0001, 0.0..=1.0));
					ui.inspect_row("Norm predict dist", &mut physics.integration_parameters.normalized_prediction_distance, (0.0001, 0.0..=1.0));
					ui.inspect_row("Norm max correct vel", &mut physics.integration_parameters.normalized_max_corrective_velocity, (0.01, 0.0..=100.0));
					ui.inspect_row("Norm max lin vel", &mut physics.integration_parameters.normalized_max_linear_velocity, (0.1, 0.0..=10000.0));
					ui.inspect_row("Num solver iter", &mut physics.integration_parameters.num_solver_iterations, 0..=100);
					ui.inspect_row("Num intern pgs iter", &mut physics.integration_parameters.num_internal_pgs_iterations, 0..=100);
					ui.inspect_row("Num intern stab iter", &mut physics.integration_parameters.num_internal_stabilization_iterations, 0..=100);
					ui.inspect_row("Max CCD substeps", &mut physics.integration_parameters.max_ccd_substeps, 0..=100);
					ui.inspect_row("Contact clustering", &mut physics.integration_parameters.contact_clustering, ());
					ui.inspect_row("Contact recycling", &mut physics.integration_parameters.contact_recycling, ());
					ui.inspect_row("Contact recycle dist", &mut physics.integration_parameters.normalized_contact_recycle_distance, (0.0001, 0.0..=1.0));
					ui.inspect_row("Frict bias pass", &mut physics.integration_parameters.friction_in_bias_pass, ());
					
					ui.label("Fiction model");
					ui.horizontal(|ui| {
						ui.radio_value(&mut physics.integration_parameters.friction_model, FrictionModel::Simplified, "Simplified");
						ui.radio_value(&mut physics.integration_parameters.friction_model, FrictionModel::Coulomb, "Coulomb");
					});
					ui.end_row();
				});
		});
	
	let rb_sel = matches!(selection, GuiSelection::RigidBody(_));
	if rb_sel {
		ui.highlight_indent(SelectionLevel::selected());
	}
	CollapsingHeader::new(format!("Rigid Bodies ({})", physics.bodies.len()))
		.id_source("Rigid Bodies")
		.open(rb_sel.then_some(true))
		.show(ui, |ui| {
			ui.reset_style();
			
			for (handle, rb) in physics.bodies.iter_mut() {
				ui.inspect_collapsing()
				  .title(id_fmt(handle.into_raw_parts().0, "RB "))
				  .show(ui, rb, (handle, application, &physics.impulse_joints));
			}
		});
	
	let col_sel = matches!(selection, GuiSelection::Collider(_));
	if col_sel {
		ui.highlight_indent(SelectionLevel::selected());
	}
	CollapsingHeader::new(format!("Colliders ({})", physics.colliders.len()))
		.id_source("Colliders")
		.open(col_sel.then_some(true))
		.show(ui, |ui| {
			for (handle, col) in physics.all_colliders_mut() {
				ui.inspect_collapsing()
				  .title(id_fmt(handle.into_raw_parts().0, "CO "))
				  .show(ui, col, (handle, application));
			}
		});
	
	let ij_sel = matches!(selection, GuiSelection::ImpulseJoint(_));
	if ij_sel {
		ui.highlight_indent(SelectionLevel::selected());
	}
	CollapsingHeader::new(format!("Joints ({})", physics.impulse_joints.len()))
		.id_source("Joints")
		.open(ij_sel.then_some(true))
		.show(ui, |ui| {
			for (handle, joint) in physics.impulse_joints.iter_mut() {
				ui.inspect_collapsing()
				  .title(id_fmt(handle.into_raw_parts().0, "IJ "))
				  .show(ui, joint, (handle, application));
			}
		});
}
