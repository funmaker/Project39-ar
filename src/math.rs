use std::any::Any;
use std::ops::{Deref, DerefMut};
use nalgebra::{Scalar, Transform, TCategory};
use simba::scalar::{SubsetOf, SupersetOf};

use crate::utils::ng_pod_impl;

pub use std::f32::consts::PI;

pub type Vec2 = nalgebra::Vector2<f32>;
pub type Vec3 = nalgebra::Vector3<f32>;
pub type Vec4 = nalgebra::Vector4<f32>;

pub type IVec2 = nalgebra::Vector2<i32>;
pub type IVec3 = nalgebra::Vector3<i32>;
pub type IVec4 = nalgebra::Vector4<i32>;

pub type Point2 = nalgebra::Point2<f32>;
pub type Point3 = nalgebra::Point3<f32>;
pub type Point4 = nalgebra::Point4<f32>;

ng_pod_impl!(Vec2, Vec3, Vec4, IVec2, IVec3, IVec4, Point2, Point3, Point4);

pub type Rot3 = nalgebra::UnitQuaternion<f32>;
pub type Translation3 = nalgebra::Translation3<f32>;
pub type Isometry3 = nalgebra::Isometry3<f32>;
pub type Similarity3 = nalgebra::Similarity3<f32>;
pub type Perspective3 = nalgebra::Perspective3<f32>;

pub type Rot2 = nalgebra::UnitComplex<f32>;
pub type Translation2 = nalgebra::Translation2<f32>;
pub type Isometry2 = nalgebra::Isometry2<f32>;
pub type Similarity2 = nalgebra::Similarity2<f32>;

pub type AMat3 = nalgebra::Affine2<f32>;
pub type AMat4 = nalgebra::Affine3<f32>;

pub type PMat3 = nalgebra::Projective2<f32>;
pub type PMat4 = nalgebra::Projective3<f32>;

pub type Mat2 = nalgebra::Matrix2<f32>;
pub type Mat3 = nalgebra::Matrix3<f32>;
pub type Mat4 = nalgebra::Matrix4<f32>;
pub type Mat3x4 = nalgebra::Matrix3x4<f32>;

ng_pod_impl!(AMat3, AMat4, PMat3, PMat4, Mat2, Mat3, Mat4, Mat3x4);

pub type Ray = rapier3d::geometry::Ray;
pub type AABB = rapier3d::geometry::AABB;

pub trait VRSlice {
	fn from_slice34(from: &[[f32; 4]; 3]) -> Self;
	fn from_slice44(from: &[[f32; 4]; 4]) -> Self;
	fn to_slice34(&self) -> [[f32; 4]; 3];
	fn to_slice44(&self) -> [[f32; 4]; 4];
}

impl VRSlice for Mat4 {
	fn from_slice34(from: &[[f32; 4]; 3]) -> Self {
		Mat4::new(
			from[0][0], from[0][1], from[0][2], from[0][3],
			from[1][0], from[1][1], from[1][2], from[1][3],
			from[2][0], from[2][1], from[2][2], from[2][3],
			       0.0,        0.0,        0.0,        1.0,
		)
	}
	
	fn from_slice44(from: &[[f32; 4]; 4]) -> Self {
		Mat4::new(
			from[0][0], from[0][1], from[0][2], from[0][3],
			from[1][0], from[1][1], from[1][2], from[1][3],
			from[2][0], from[2][1], from[2][2], from[2][3],
			from[3][0], from[3][1], from[3][2], from[3][3],
		)
	}
	
	fn to_slice34(&self) -> [[f32; 4]; 3] {
		[
			[self.m11, self.m12, self.m13, self.m14],
			[self.m21, self.m22, self.m23, self.m24],
			[self.m31, self.m32, self.m33, self.m34],
		]
	}
	
	fn to_slice44(&self) -> [[f32; 4]; 4] {
		[
			[self.m11, self.m12, self.m13, self.m14],
			[self.m21, self.m22, self.m23, self.m24],
			[self.m31, self.m32, self.m33, self.m34],
			[self.m41, self.m42, self.m43, self.m44]
		]
	}
}

impl<C: TCategory> VRSlice for Transform<f32, C, 3> {
	fn from_slice34(from: &[[f32; 4]; 3]) -> Self {
		Mat4::from_slice34(from).to_subset_lossy()
	}
	
	fn from_slice44(from: &[[f32; 4]; 4]) -> Self {
		Mat4::from_slice44(from).to_subset_lossy()
	}
	
	fn to_slice34(&self) -> [[f32; 4]; 3] {
		self.to_homogeneous().to_slice34()
	}
	
	fn to_slice44(&self) -> [[f32; 4]; 4] {
		self.to_homogeneous().to_slice44()
	}
}

pub fn aabb_from_points<'a, I>(pts: I) -> AABB
	where
		I: IntoIterator<Item = Point3>,
{
	let mut it = pts.into_iter();
	
	let p0 = it.next().expect(
		"Point cloud AABB construction: the input iterator should yield at least one point.",
	);
	let mut min = p0;
	let mut max = p0;
	
	for pt in it {
		min = min.inf(&pt);
		max = max.sup(&pt);
	}
	
	AABB::new(min, max)
}

pub fn cast_ray_on_plane(plane: Isometry3, ray: Ray) -> Option<Point3> {
	let norm = plane.transform_vector(&Vec3::z_axis());
	let origin = plane.transform_point(&Point3::origin());
	let toi = (origin - ray.origin).dot(&norm) / ray.dir.dot(&norm);
	
	if toi.is_nan() || toi < 0.0 {
		None
	} else {
		let intersection = ray.point_at(toi);
		Some(plane.inverse_transform_point(&intersection))
	}
}

pub fn face_towards_lossy(dir: Vec3) -> Rot3 {
	if dir.cross(&Vec3::y_axis()).magnitude_squared() <= f32::EPSILON {
		Rot3::face_towards(&dir, &Vec3::z_axis())
	} else {
		Rot3::face_towards(&dir, &Vec3::y_axis())
	}
}

pub fn face_upwards_lossy(dir: Vec3) -> Rot3 {
	if dir.cross(&-Vec3::y_axis()).magnitude_squared() < f32::EPSILON {
		Rot3::identity()
	} else {
		Rot3::face_towards(&dir.cross(&-Vec3::y_axis()).cross(&dir), &dir)
	}
}

// Using ZXY euler sequence
// Thanks for help kirsh168
pub fn to_euler(rot: Rot3) -> (f32, f32, f32) {
	let m11 = 2.0 * (rot.w * rot.w + rot.i * rot.i) - 1.0;
	// let m12 = 2.0 * (rot.i * rot.j - rot.w * rot.k);
	let m13 = 2.0 * (rot.i * rot.k + rot.w * rot.j);
	
	let m21 = 2.0 * (rot.i * rot.j + rot.w * rot.k);
	let m22 = 2.0 * (rot.w * rot.w + rot.j * rot.j) - 1.0;
	let m23 = 2.0 * (rot.j * rot.k - rot.w * rot.i);
	
	let m31 = 2.0 * (rot.i * rot.k - rot.w * rot.j);
	// let m32 = 2.0 * (rot.j * rot.k + rot.w * rot.i);
	let m33 = 2.0 * (rot.w * rot.w + rot.k * rot.k) - 1.0;
	
	let pitch = -m23.clamp(-1.0, 1.0).asin();
	let gimbal_lock = pitch.abs() > PI / 2.0 - 0.001;
	
	let yaw = if gimbal_lock {
		f32::atan2(-m31, m11)
	} else {
		f32::atan2(m13, m33)
	};
	
	let xy_proj = m21 / pitch.cos();
	let roll = if gimbal_lock {
		0.0
	} else if m22 < 0.0 {
		PI.copysign(m21) - xy_proj.clamp(-1.0, 1.0).asin() // Upside down
	} else {
		xy_proj.clamp(-1.0, 1.0).asin()
	};
	
	(pitch, yaw, roll)
}

// Using ZXY euler sequence
pub fn from_euler(pitch: f32, yaw: f32, roll: f32) -> Rot3 {
	let x = Rot3::from_axis_angle(&Vec3::x_axis(), pitch);
	let y = Rot3::from_axis_angle(&Vec3::y_axis(), yaw);
	let z = Rot3::from_axis_angle(&Vec3::z_axis(), roll);
	
	y * x * z
}

// Translates OpenGL projection matrix to Vulkan
// Can't be const because Mat4::new is not const fn or something
pub fn projective_clip() -> AMat4 {
	AMat4::from_matrix_unchecked(Mat4::new(
		1.0, 0.0, 0.0, 0.0,
		0.0,-1.0, 0.0, 0.0,
		0.0, 0.0, 0.5, 0.5,
		0.0, 0.0, 0.0, 1.0,
	))
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color(Vec4);

impl Color {
	pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Color(vector!(r, g, b, a)) }
	
	pub fn dblack()   -> Self { Color(vector!(0.0, 0.0, 0.0, 1.0)) }
	pub fn dred()     -> Self { Color(vector!(0.5, 0.0, 0.0, 1.0)) }
	pub fn dgreen()   -> Self { Color(vector!(0.0, 0.6, 0.0, 1.0)) }
	pub fn dyellow()  -> Self { Color(vector!(0.1, 0.5, 0.0, 1.0)) }
	pub fn dblue()    -> Self { Color(vector!(0.0, 0.0, 0.5, 1.0)) }
	pub fn dmagenta() -> Self { Color(vector!(0.6, 0.0, 0.6, 1.0)) }
	pub fn dcyan()    -> Self { Color(vector!(0.0, 0.6, 0.6, 1.0)) }
	pub fn dwhite()   -> Self { Color(vector!(0.8, 0.8, 0.8, 1.0)) }
	
	pub fn black()    -> Self { Color(vector!(0.5, 0.5, 0.5, 1.0)) }
	pub fn red()      -> Self { Color(vector!(1.0, 0.0, 0.0, 1.0)) }
	pub fn green()    -> Self { Color(vector!(0.0, 1.0, 0.0, 1.0)) }
	pub fn yellow()   -> Self { Color(vector!(1.0, 1.0, 0.0, 1.0)) }
	pub fn blue()     -> Self { Color(vector!(0.0, 0.0, 1.0, 1.0)) }
	pub fn magenta()  -> Self { Color(vector!(1.0, 0.0, 1.0, 1.0)) }
	pub fn cyan()     -> Self { Color(vector!(0.0, 1.0, 1.0, 1.0)) }
	pub fn white()    -> Self { Color(vector!(1.0, 1.0, 1.0, 1.0)) }
	
	pub fn full_black()  -> Self { Color(vector!(0.0, 0.0, 0.0, 1.0)) }
	pub fn full_white()  -> Self { Color(vector!(1.0, 1.0, 1.0, 1.0)) }
	pub fn transparent() -> Self { Color(vector!(0.0, 0.0, 0.0, 0.0)) }
	
	pub fn opactiy(self, opacity: f32) -> Self { Color(self.0 * opacity) }
	pub fn lightness(self, lightness: f32) -> Self { Color(
		if lightness < 1.0 {
			self.0.component_mul(&vector!(lightness, lightness, lightness, 1.0))
		} else {
			self.0.component_div(&vector!(lightness, lightness, lightness, 1.0)) + vector!(1.0 - 1.0 / lightness, 1.0 - 1.0 / lightness, 1.0 - 1.0 / lightness, 0.0)
		}
	) }
}

impl Deref for Color {
	type Target = Vec4;
	
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Color {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}


pub trait SubsetOfLossy<T> {
	fn from_superset_lossy(element: &T) -> Self;
}

impl<T, S> SubsetOfLossy<S> for T where T: SubsetOf<S> + Any {
	fn from_superset_lossy(element: &S) -> Self  {
		Self::from_superset(element).unwrap_or_else(|| {
			use std::any::type_name;
			eprintln!("Invalid upcast from {} to {}", type_name::<S>(), type_name::<T>());
			Self::from_superset_lossy(element)
		})
	}
}

pub trait SuperSetOfLossy<T>: SupersetOf<T> + Any {
	fn to_subset_lossy(&self) -> T;
}

impl<T, S> SuperSetOfLossy<S> for T where T: SupersetOf<S> + Any {
	fn to_subset_lossy(&self) -> S {
		self.to_subset().unwrap_or_else(|| {
			use std::any::type_name;
			eprintln!("Invalid downcast from {} to {}", type_name::<T>(), type_name::<S>());
			self.to_subset_unchecked()
		})
	}
}

pub trait IntoArray<T> {
	fn into_array(self) -> T;
}

impl<T> IntoArray<T> for T {
	fn into_array(self) -> T { self }
}

impl<T: Clone> IntoArray<T> for &T {
	fn into_array(self) -> T { self.clone() }
}

impl<N: Scalar> IntoArray<[N; 2]> for  nalgebra::Vector2<N> { fn into_array(self) -> [N; 2] { [self.x.clone(), self.y.clone()] } }
impl<N: Scalar> IntoArray<[N; 2]> for &nalgebra::Vector2<N> { fn into_array(self) -> [N; 2] { [self.x.clone(), self.y.clone()] } }

impl<N: Scalar> IntoArray<[N; 3]> for  nalgebra::Vector3<N> { fn into_array(self) -> [N; 3] { [self.x.clone(), self.y.clone(), self.z.clone()] } }
impl<N: Scalar> IntoArray<[N; 3]> for &nalgebra::Vector3<N> { fn into_array(self) -> [N; 3] { [self.x.clone(), self.y.clone(), self.z.clone()] } }

impl<N: Scalar> IntoArray<[N; 4]> for  nalgebra::Vector4<N> { fn into_array(self) -> [N; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }
impl<N: Scalar> IntoArray<[N; 4]> for &nalgebra::Vector4<N> { fn into_array(self) -> [N; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }

impl<N: Scalar> IntoArray<[N; 2]> for  nalgebra::Point2<N> { fn into_array(self) -> [N; 2] { [self.x.clone(), self.y.clone()] } }
impl<N: Scalar> IntoArray<[N; 2]> for &nalgebra::Point2<N> { fn into_array(self) -> [N; 2] { [self.x.clone(), self.y.clone()] } }

impl<N: Scalar> IntoArray<[N; 3]> for  nalgebra::Point3<N> { fn into_array(self) -> [N; 3] { [self.x.clone(), self.y.clone(), self.z.clone()] } }
impl<N: Scalar> IntoArray<[N; 3]> for &nalgebra::Point3<N> { fn into_array(self) -> [N; 3] { [self.x.clone(), self.y.clone(), self.z.clone()] } }

impl<N: Scalar> IntoArray<[N; 4]> for  nalgebra::Point4<N> { fn into_array(self) -> [N; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }
impl<N: Scalar> IntoArray<[N; 4]> for &nalgebra::Point4<N> { fn into_array(self) -> [N; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }

impl IntoArray<[f32; 4]> for  Color { fn into_array(self) -> [f32; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }
impl IntoArray<[f32; 4]> for &Color { fn into_array(self) -> [f32; 4] { [self.x.clone(), self.y.clone(), self.z.clone(), self.w.clone()] } }
