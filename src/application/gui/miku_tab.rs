use egui::Ui;

use crate::component::ComponentBase;
use crate::component::miku::Miku;
use crate::utils::ExUi;
use super::Application;

pub fn miku_ui(miku: &Miku, ui: &mut Ui, application: &Application) {
	ui.inspect(miku, application);
	
	ui.separator();
	
	let model = match miku.model.get(application) {
		Some(model) => model,
		None => return,
	};
	
	let selected_bone = application.gui_selections().selection.mmd_bone();
	
	if let Some(selected_bone) = selected_bone {
		let rb = model.state()
		              .rigid_bodies(application)
		              .find(|rb| rb.bone == selected_bone);
		
		if let Some(rb) = rb {
			if let Some(joint) = rb.joint.get(application) {
				ui.inspect_collapsing()
				  .title("MMD Joint")
				  .default_open(true)
				  .show(ui, joint, application);
			}
			
			ui.inspect_collapsing()
			  .title("MMD Rigid Body")
			  .default_open(true)
			  .show(ui, rb, application);
		}
		
		if let Some(bone) = model.state_mut().bones.get_mut(selected_bone) {
			ui.inspect_collapsing()
			  .title("Bone")
			  .show(ui, bone, application);
		}
		
		if let Some(rb) = rb {
			ui.inspect_collapsing()
			  .title("Entity")
			  .show(ui, rb.entity(application), application);
		}
	}
}
