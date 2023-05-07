use rapier3d::dynamics::RigidBodyType;
use rapier3d::pipeline::QueryFilter;

use crate::debug;
use crate::application::{Hand, Application};
use crate::application::entity::EntityBuilder;
use crate::component::seat::Seat;
use crate::math::{Ray, Similarity3, Color, Rot3, Isometry3, Vec3, cast_ray_on_plane, face_upwards_lossy};
use crate::renderer::RenderContext;
use super::ToolGun;
use super::tool::{Tool, ToolError};


const MENU_SCALE: f32 = 0.2;
const MENU_SPACING: f32 = 0.25;
const MENU_DISTANCE: f32 = 0.25;

pub struct Spawner {
	menu_pos: Option<Isometry3>,
	prop_idx: usize,
	select_idx: Option<usize>,
	ghost_pos: Option<Isometry3>,
}

impl Spawner {
	pub fn new() -> Self {
		Spawner {
			menu_pos: None,
			prop_idx: 0,
			select_idx: None,
			ghost_pos: None,
		}
	}
}

impl Tool for Spawner {
	fn name(&self) -> &str {
		"Spawner"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		self.ghost_pos = None;
		self.select_idx = None;
		
		if application.input.use2_btn(hand).down {
			if self.menu_pos.is_some() {
				self.menu_pos = None;
			} else {
				self.menu_pos = Some(Isometry3::face_towards(&ray.point_at(MENU_DISTANCE), &ray.origin, &Vec3::y_axis()));
			}
		}
		
		if let Some(menu_pos) = self.menu_pos {
			if let Some(select_pos) = cast_ray_on_plane(menu_pos, ray) {
				let size = toolgun.prop_collection.props.len();
				let row_size = (size as f32).sqrt().ceil();
				let hsize = row_size / 2.0 - 0.5;
				let x = (select_pos.x / MENU_SPACING + hsize).round();
				let y = (hsize - select_pos.y / MENU_SPACING).round();
				
				if x >= 0.0 && x < row_size && y >= 0.0 && y < row_size {
					let idx = (x + y * row_size) as usize;
					self.select_idx = Some(idx);
					
					if application.input.fire_btn(hand).down {
						self.prop_idx = idx;
					}
				}
			}
			
			if application.input.fire_btn(hand).down {
				self.menu_pos = None;
			}
		} else {
			let result = {
				let physics = &*application.physics.borrow();
				physics.query_pipeline.cast_ray_and_get_normal(&physics.rigid_body_set, &physics.collider_set, &ray, 9999.0, false, QueryFilter::new())
			};
			
			if let Some((_, intersection)) = result {
				if let Some(prop) = toolgun.prop_collection.props.get(self.prop_idx) {
					let hit_point = ray.point_at(intersection.toi);
					let offset = prop.model.aabb().mins.y;
					
					let position = Isometry3::from_parts(
						(hit_point - intersection.normal * offset).into(),
						face_upwards_lossy(intersection.normal),
					);
					
					self.ghost_pos = Some(position);
					
					if application.input.fire_btn(hand).down {
						toolgun.fire(application);
						
						let mut builder = EntityBuilder::new(&prop.name)
							.rigid_body_type(RigidBodyType::Dynamic)
							.position(position)
							.component(prop.model.clone())
							.collider(prop.collider.clone());
						
						if let Some(seat) = prop.seat {
							builder = builder.component(Seat::new(seat));
						}
						
						application.add_entity(builder.build());
					}
				}
			}
		}
		
		
		Ok(())
	}
	
	fn render(&mut self, toolgun: &ToolGun, context: &mut RenderContext) -> Result<(), ToolError> {
		if let Some(ghost_pos) = self.ghost_pos {
			if let Some(prop) = toolgun.prop_collection.props.get(self.prop_idx) {
				prop.model.render_impl(Similarity3::from_isometry(ghost_pos, 1.0), Color::full_white().opactiy(0.25), context)?;
			}
		}
		
		if let Some(menu_pos) = self.menu_pos {
			let size = toolgun.prop_collection.props.len();
			let row_size = (size as f32).sqrt().ceil() as usize;
			
			for (id, prop) in toolgun.prop_collection.props.iter().enumerate() {
				let x = (id % row_size) as f32;
				let y = (id / row_size) as f32;
				let hsize = row_size as f32 / 2.0 - 0.5;
				let pos = vector!((x - hsize) * MENU_SPACING,
				                  (hsize - y) * MENU_SPACING,
				                  0.0);
				let size = MENU_SCALE / prop.model.aabb().extents().max();
				
				let transform = menu_pos * Similarity3::from_parts(pos.into(), Rot3::from_euler_angles(0.0, 0.0, 0.0), size);
				
				let color = if self.select_idx == Some(id) {
					Color::cyan()
				} else {
					Color::dwhite().opactiy(0.75)
				};
				
				prop.model.render_impl(transform, color, context)?;
				
				if let Some(tip) = &prop.tip {
					debug::draw_text(tip, transform, debug::DebugOffset::top(0.0, 8.0), 32.0, color);
				}
			}
		}
		
		Ok(())
	}
}
