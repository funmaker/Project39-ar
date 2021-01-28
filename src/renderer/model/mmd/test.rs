use std::io::BufReader;
use std::fs::File;
use cgmath::{Vector3, Vector4, Matrix4};
use image::ImageFormat;

use crate::renderer::Renderer;
use crate::application::entity::{Bone, BoneConnection};
use super::{MMDModel, Vertex, MaterialInfo};

#[allow(dead_code)]
pub fn test_model(renderer: &mut Renderer) -> MMDModel<u16> {
	let mut vertices = vec![];
	let mut indices = vec![];
	let bones_num = 3;
	let height = 4.0;
	
	let mut make_wall = |from: Vector3<f32>, to: Vector3<f32>, normal: [f32; 3], divs: usize, bones: usize| {
		let base_index = vertices.len();
		
		for d in 0..=divs {
			let part = d as f32 / divs as f32;
			
			let bone = (part * bones as f32).trunc() as u32;
			let bone_w = 1.0 - (part * bones as f32).fract();
			
			vertices.push(Vertex::new([from.x, (to.y - from.y) * part + from.y, from.z], normal, [0.0, part], 1.0, [bone, bone + 1, 0, 0], [bone_w, 1.0 - bone_w, 0.0, 0.0]));
			vertices.push(Vertex::new([  to.x, (to.y - from.y) * part + from.y,   to.z], normal, [1.0, part], 1.0, [bone, bone + 1, 0, 0], [bone_w, 1.0 - bone_w, 0.0, 0.0]));
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
	
	make_wall([-0.2, 0.0, -0.2].into(), [ 0.2, height, -0.2].into(), [ 0.0, 0.0, -1.0], 50, bones_num + 1);
	make_wall([-0.2, 0.0,  0.2].into(), [-0.2, height, -0.2].into(), [-1.0, 0.0,  0.0], 50, bones_num + 1);
	make_wall([ 0.2, 0.0,  0.2].into(), [-0.2, height,  0.2].into(), [ 0.0, 0.0,  1.0], 50, bones_num + 1);
	make_wall([ 0.2, 0.0, -0.2].into(), [ 0.2, height,  0.2].into(), [ 1.0, 0.0,  0.0], 50, bones_num + 1);
	
	let mut model = MMDModel::new(&vertices, &indices, renderer).unwrap();
	
	let texture_reader = BufReader::new(File::open("models/missing.png").unwrap());
	let image = image::load(texture_reader, ImageFormat::Png).unwrap();
	
	let texture = model.add_texture(image, renderer).unwrap();
	
	let material_info = MaterialInfo {
		color: [1.0, 1.0, 1.0, 1.0],
		specular: [1.0, 1.0, 1.0],
		specularity: 1.0,
		ambient: [0.0, 0.0, 0.0],
		sphere_mode: 0
	};
	
	model.add_sub_mesh(0..indices.len(), material_info, Some(texture), None, None, false, true, None, renderer).unwrap();
	
	model.add_bone(Bone::new("Root",
	                         None,
	                         Vector4::new(0.0, 1.0, 1.0, 1.0),
	                         Matrix4::from_translation(Vector3::new(0.0, 0.0, 0.0)),
	                         Vector3::new(0.0, 0.0, 0.0),
	                         true,
	                         BoneConnection::Bone(1)));
	
	for id in 1..=bones_num {
		model.add_bone(Bone::new("Bend",
		                         Some(id - 1),
		                         Vector4::new(0.0, 1.0, 1.0, 1.0),
		                         Matrix4::from_translation(Vector3::new(0.0, height / (bones_num + 1) as f32, 0.0)),
		                         Vector3::new(0.0, height / (bones_num + 1) as f32 * id as f32, 0.0),
		                         true,
		                         BoneConnection::Bone(id + 1)));
	}
	
	model.add_bone(Bone::new("Tip",
	                         Some(bones_num),
	                         Vector4::new(0.0, 1.0, 1.0, 1.0),
	                         Matrix4::from_translation(Vector3::new(0.0, 1.0, 0.0)),
	                         Vector3::new(0.0, height, 0.0),
	                         true,
	                         BoneConnection::None));
	
	model
}
