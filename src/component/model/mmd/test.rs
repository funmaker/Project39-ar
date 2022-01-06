use std::io::BufReader;
use std::fs::File;
use std::sync::Arc;
use image::ImageFormat;

use crate::renderer::Renderer;
use crate::math::{Color, Vec3};
use super::{MMDModel, Vertex, MMDBone, BoneConnection, shared::MMDModelShared, shared::SubMeshDesc};

#[allow(dead_code)]
pub fn test_model(renderer: &mut Renderer) -> MMDModel {
	let mut vertices = vec![];
	let mut indices = vec![];
	let bones_num = 1;
	let height = 2.0;
	
	let mut make_wall = |from: Vec3, to: Vec3, normal: Vec3, divs: usize, bones: usize| {
		let base_index = vertices.len();
		
		for d in 0..=divs {
			let part = d as f32 / divs as f32;
			
			let bone = (part * bones as f32).trunc() as u32;
			let bone_w = 1.0 - (part * bones as f32).fract();
			
			vertices.push(Vertex::new([from.x, (to.y - from.y) * part + from.y, from.z], normal.clone(), [0.0, part], 1.0, [bone, bone + 1, 0, 0], [bone_w, 1.0 - bone_w, 0.0, 0.0]));
			vertices.push(Vertex::new([  to.x, (to.y - from.y) * part + from.y,   to.z], normal.clone(), [1.0, part], 1.0, [bone, bone + 1, 0, 0], [bone_w, 1.0 - bone_w, 0.0, 0.0]));
		}
		
		for d in 0..divs {
			indices.push((base_index + d * 2 + 0) as u16);
			indices.push((base_index + d * 2 + 1) as u16);
			indices.push((base_index + d * 2 + 3) as u16);
			indices.push((base_index + d * 2 + 0) as u16);
			indices.push((base_index + d * 2 + 3) as u16);
			indices.push((base_index + d * 2 + 2) as u16);
		}
	};
	
	make_wall([-0.2, 0.0, -0.2].into(), [ 0.2, height, -0.2].into(), [ 0.0, 0.0, -1.0].into(), 50, bones_num);
	make_wall([-0.2, 0.0,  0.2].into(), [-0.2, height, -0.2].into(), [-1.0, 0.0,  0.0].into(), 50, bones_num);
	make_wall([ 0.2, 0.0,  0.2].into(), [-0.2, height,  0.2].into(), [ 0.0, 0.0,  1.0].into(), 50, bones_num);
	make_wall([ 0.2, 0.0, -0.2].into(), [ 0.2, height,  0.2].into(), [ 1.0, 0.0,  0.0].into(), 50, bones_num);
	
	let indices_range = 0 .. indices.len() as u32;
	
	let mut model = MMDModelShared::new(vertices, indices);
	
	let texture_reader = BufReader::new(File::open("models/missing.png").unwrap());
	let image = image::load(texture_reader, ImageFormat::Png).unwrap();
	
	model.add_texture(image);
	
	model.add_sub_mesh(SubMeshDesc {
		range: indices_range,
		texture: Some(0),
		toon: None,
		sphere_map: None,
		color: vector![1.0, 1.0, 1.0, 1.0],
		specular: vector![1.0, 1.0, 1.0],
		specularity: 1.0,
		ambient: vector![0.0, 0.0, 0.0],
		sphere_mode: 0,
		no_cull: false,
		opaque: true,
		edge: None
	});
	
	model.add_bone(MMDBone::new("Root",
	                            None,
	                            Color::cyan(),
	                            &vector!(0.0, 0.0, 0.0),
	                            &vector!(0.0, 0.0, 0.0),
	                            true,
	                            BoneConnection::Bone(1)));
	
	for id in 1..=bones_num {
		model.add_bone(MMDBone::new("Bend",
		                            Some(id - 1),
		                            Color::cyan(),
		                            &vector!(0.0, height / (bones_num + 1) as f32 * id as f32, 0.0),
		                            &vector!(0.0, height / (bones_num + 1) as f32, 0.0),
		                            true,
		                            BoneConnection::Bone(id + 1)));
	}
	
	model.add_bone(MMDBone::new("Tip",
	                            Some(bones_num),
	                            Color::cyan(),
	                            &vector!(0.0, height, 0.0),
	                            &vector!(0.0, height / (bones_num + 1) as f32, 0.0),
	                            true,
	                            BoneConnection::None));
	
	let model = model.build(renderer).unwrap();
	
	MMDModel::new(Arc::new(model), renderer).unwrap()
}
