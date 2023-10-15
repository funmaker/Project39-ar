use egui::{Grid, RichText, Ui, WidgetText};

use crate::application::{Application, EntityRef};
use crate::math::{Color, Point3, Similarity3, Translation3, Isometry3};
use crate::utils::{ExUi, id_fmt, InspectMut, InspectObject};
use super::super::super::ComponentRef;
use super::MMDModel;
use super::shared::{BoneDesc, BoneConnection};


#[derive(Debug, Clone)]
pub struct MMDBone {
	pub id: usize,
	pub model: ComponentRef<MMDModel>,
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub inv_model_transform: Translation3,
	pub local_transform: Translation3,
	pub anim_transform: Similarity3,
	pub transform_override: Option<Similarity3>,
	pub display: bool,
	pub connection: BoneConnection,
	pub rigid_body: EntityRef,
	pub inv_rigid_body_transform: Isometry3,
}

impl MMDBone {
	pub fn origin(&self) -> Point3 {
		self.inv_model_transform.inverse_transform_point(&Point3::origin())
	}
	
	pub fn attach_rigid_body(&mut self, rigid_body: EntityRef, model_pos: Isometry3) -> &mut Self {
		self.rigid_body = rigid_body;
		self.inv_rigid_body_transform = (self.inv_model_transform * model_pos).inverse();
		self
	}
}

impl From<&BoneDesc> for MMDBone {
	fn from(desc: &BoneDesc) -> Self {
		MMDBone {
			id: desc.id,
			model: ComponentRef::null(),
			name: desc.name.clone(),
			parent: desc.parent,
			color: desc.color,
			inv_model_transform: (-desc.model_pos).into(),
			local_transform: desc.local_pos.into(),
			anim_transform: Similarity3::identity(),
			transform_override: None,
			display: desc.display,
			connection: desc.connection,
			rigid_body: EntityRef::null(),
			inv_rigid_body_transform: Isometry3::identity(),
		}
	}
}

impl InspectMut for MMDBone {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(&mut self, ui: &mut Ui, application: Self::Options<'_>) {
		Grid::new("MMDBone")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.label("ID");
				bone_button(self.model.clone(), self.id, ui, application);
				ui.end_row();
				
				ui.inspect_row("Model", &self.model, application);
				ui.inspect_row("Name", &self.name, ());
				
				ui.label("Parent");
				if let Some(parent) = self.parent {
					bone_button(self.model.clone(), parent, ui, application);
				} else {
					ui.label(RichText::new("NULL").monospace().italics());
				}
				ui.end_row();
				
				ui.inspect_row("Color", &mut self.color, ());
				ui.inspect_row("Inv Model", &mut self.inv_model_transform, ());
				ui.inspect_row("Local", &mut self.local_transform, ());
				ui.inspect_row("Animation", &mut self.anim_transform, ());
				ui.inspect_row("Override", &mut self.transform_override, ());
				ui.inspect_row("Display", &mut self.display, ());
				
				ui.label("Connection");
				match &mut self.connection {
					BoneConnection::None => { ui.label(RichText::new("NONE").monospace().italics()); },
					BoneConnection::Bone(bone) => bone_button(self.model.clone(), *bone, ui, application),
					BoneConnection::Offset(offset) => ui.inspect(offset, ()),
				}
				ui.end_row();
				
				ui.inspect_row("Rigid Body", &self.rigid_body, application);
				ui.inspect_row("Inv Body", &mut self.inv_rigid_body_transform, ());
			});
	}
}

impl InspectObject for &mut MMDBone {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().component() == self.model
			&& application.get_selection().mmd_bone() == Some(self.id)
	}
	
	fn inspect_header(&self, _application: &Self::Options<'_>) -> WidgetText {
		self.name.clone().into()
	}
	
	fn inspect_uid(&self, _application: &Self::Options<'_>) -> u64 {
		use std::hash::{Hasher, Hash};
		use std::any::Any;
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		(**self).type_id().hash(&mut s);
		self.model.inner().hash(&mut s);
		self.id.hash(&mut s);
		s.finish()
	}
}

fn bone_button(model: ComponentRef<MMDModel>, bone: usize, ui: &mut Ui, application: &Application) {
	if ui.button(id_fmt(bone, "MB ")).clicked() {
		application.select((model, bone));
	}
}
