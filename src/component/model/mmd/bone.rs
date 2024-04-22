use egui::{Grid, RichText, Ui, WidgetText};

use crate::application::{Application, EntityRef};
use crate::math::{Color, Point3, Similarity3, Translation3, Isometry3};
use crate::utils::{ExUi, id_fmt, Inspect, InspectMut, InspectObject};
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
	pub rigid_body: EntityRef,
	pub model_transform: Translation3,
	pub local_transform: Translation3,
	pub anim_transform: Similarity3,
	pub override_transform: Option<Similarity3>,
	pub rigid_body_transform: Isometry3,
	pub display: bool,
	pub connection: BoneConnection,
}

impl MMDBone {
	pub fn origin(&self) -> Point3 {
		self.model_transform * Point3::origin()
	}
	
	pub fn attach_rigid_body(&mut self, rigid_body: EntityRef, model_pos: Isometry3) -> &mut Self {
		self.rigid_body = rigid_body;
		self.rigid_body_transform = self.model_transform.inverse() * model_pos;
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
			rigid_body: EntityRef::null(),
			model_transform: desc.model_pos.into(),
			local_transform: desc.local_pos.into(),
			anim_transform: Similarity3::identity(),
			override_transform: None,
			rigid_body_transform: Isometry3::identity(),
			display: desc.display,
			connection: desc.connection,
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
				ui.inspect_row("ID", (self.model.clone(), self.id), application);
				ui.inspect_row("Model", &self.model, application);
				ui.inspect_row("Name", &self.name, ());
				
				if let Some(parent) = self.parent {
					ui.inspect_row("Parent", (self.model.clone(), parent), application);
				} else {
					ui.inspect_row("Parent", RichText::new("NULL").monospace().italics(), ());
				}
				
				ui.inspect_row("Color", &mut self.color, ());
				ui.inspect_row("Rigid Body", &self.rigid_body, application);
				ui.inspect_row("Model Tr", &mut self.model_transform, ());
				ui.inspect_row("Local Tr", &mut self.local_transform, ());
				ui.inspect_row("Animation Tr", &mut self.anim_transform, ());
				ui.inspect_row("Override Tr", &mut self.override_transform, ());
				ui.inspect_row("Body Tr", &mut self.rigid_body_transform, ());
				ui.inspect_row("Display", &mut self.display, ());
				
				match &mut self.connection {
					BoneConnection::None =>           ui.inspect_row("Connection", RichText::new("NULL").monospace().italics(), ()),
					BoneConnection::Bone(bone) =>     ui.inspect_row("Connection", (self.model.clone(), *bone), application),
					BoneConnection::Offset(offset) => ui.inspect_row("Connection", offset, ()),
				}
			});
	}
}

impl InspectObject for &mut MMDBone {
	fn is_selected(&self, _application: &Self::Options<'_>) -> bool {
		// application.get_selection().component() == self.model
		// 	&& application.get_selection().mmd_bone() == Some(self.id)
		false
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

impl Inspect for (ComponentRef<MMDModel>, usize) {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: Self::Options<'_>) {
		if ui.button(id_fmt(self.1, "MB ")).clicked() {
			application.select(self);
		}
	}
}
