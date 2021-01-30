use nalgebra::{Transform, U2, U3, DimNameAdd, U1, DefaultAllocator, DimNameSum, TCategory, RealField, Scalar};
use nalgebra::allocator::Allocator;
use derive_deref::{Deref, DerefMut};
use simba::scalar::{SubsetOf, SupersetOf};
use std::any::Any;

pub type Vec2 = nalgebra::Vector2<f32>;
pub type Vec3 = nalgebra::Vector3<f32>;
pub type Vec4 = nalgebra::Vector4<f32>;

pub type Point2 = nalgebra::Point2<f32>;
pub type Point3 = nalgebra::Point3<f32>;
pub type Point4 = nalgebra::Point4<f32>;

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

pub trait ToTransform where DefaultAllocator: Allocator<Self::N, DimNameSum<Self::D, U1>, DimNameSum<Self::D, U1>> {
	type N: RealField;
	type D: DimNameAdd<U1>;
	fn to_transform<T>(&self) -> Transform<Self::N, Self::D, T>
	                             where T: TCategory;
}

impl<N: RealField> ToTransform for nalgebra::UnitComplex<N> {
	type N = N;
	type D = U2;
	fn to_transform<T>(&self) -> Transform<N, U2, T>
	                             where T: TCategory {
		Transform::from_matrix_unchecked(self.to_homogeneous())
	}
}

impl<N: RealField> ToTransform for nalgebra::UnitQuaternion<N> {
	type N = N;
	type D = U3;
	fn to_transform<T>(&self) -> Transform<N, U3, T>
	                             where T: TCategory {
		Transform::from_matrix_unchecked(self.to_homogeneous())
	}
}

impl<N, D, R> ToTransform for nalgebra::Isometry<N, D, R>
	where N: RealField,
	      D: DimNameAdd<U1>,
	      R: SubsetOf<nalgebra::MatrixN<N, DimNameSum<D, U1>>>,
	      DefaultAllocator: Allocator<N, D>,
	      DefaultAllocator: Allocator<N, DimNameSum<D, U1>, DimNameSum<D, U1>> {
	type N = N;
	type D = D;
	fn to_transform<T>(&self) -> Transform<N, D, T>
	                             where T: TCategory {
		Transform::from_matrix_unchecked(self.to_homogeneous())
	}
}

impl<N, D, R> ToTransform for nalgebra::Similarity<N, D, R>
	where N: RealField,
	      D: DimNameAdd<U1>,
	      R: SubsetOf<nalgebra::MatrixN<N, DimNameSum<D, U1>>>,
	      DefaultAllocator: Allocator<N, D>,
	      DefaultAllocator: Allocator<N, DimNameSum<D, U1>, DimNameSum<D, U1>> {
	type N = N;
	type D = D;
	fn to_transform<T>(&self) -> Transform<N, D, T>
	                             where T: TCategory {
		Transform::from_matrix_unchecked(self.to_homogeneous())
	}
}

impl<N: RealField> ToTransform for [[N; 4]; 3] {
	type N = N;
	type D = U3;
	fn to_transform<T>(&self) -> Transform<N, U3, T>
	                             where T: TCategory {
		Transform::from_matrix_unchecked(
			nalgebra::Matrix4::<N>::new(self[0][0], self[0][1], self[0][2], self[0][3],
			                            self[1][0], self[1][1], self[1][2], self[1][3],
			                            self[2][0], self[2][1], self[2][2], self[2][3],
			                             N::zero(),  N::zero(),  N::zero(),   N::one())
		)
	}
}

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
			[self.m11, self.m21, self.m31, self.m41],
			[self.m12, self.m22, self.m32, self.m42],
			[self.m13, self.m23, self.m33, self.m43],
		]
	}
	
	fn to_slice44(&self) -> [[f32; 4]; 4] {
		[
			[self.m11, self.m21, self.m31, self.m41],
			[self.m12, self.m22, self.m32, self.m42],
			[self.m13, self.m23, self.m33, self.m43],
			[self.m14, self.m24, self.m34, self.m44],
		]
	}
}

#[derive(Deref, DerefMut, Clone, Debug, PartialEq)]
pub struct Color(Vec4);

impl Color {
	pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Color(Vec4::new(r, g, b, a)) }
	
	pub fn dblack()   -> Self { Color(Vec4::new(0.0, 0.0, 0.0, 1.0)) }
	pub fn dred()     -> Self { Color(Vec4::new(0.5, 0.0, 0.0, 1.0)) }
	pub fn dgreen()   -> Self { Color(Vec4::new(0.0, 0.6, 0.0, 1.0)) }
	pub fn dyellow()  -> Self { Color(Vec4::new(0.1, 0.5, 0.0, 1.0)) }
	pub fn dblue()    -> Self { Color(Vec4::new(0.0, 0.0, 0.5, 1.0)) }
	pub fn dmagenta() -> Self { Color(Vec4::new(0.6, 0.0, 0.6, 1.0)) }
	pub fn dcyan()    -> Self { Color(Vec4::new(0.0, 0.6, 0.6, 1.0)) }
	pub fn dwhite()   -> Self { Color(Vec4::new(0.8, 0.8, 0.8, 1.0)) }
	
	pub fn black()    -> Self { Color(Vec4::new(0.5, 0.5, 0.5, 1.0)) }
	pub fn red()      -> Self { Color(Vec4::new(1.0, 0.0, 0.0, 1.0)) }
	pub fn green()    -> Self { Color(Vec4::new(0.0, 1.0, 0.0, 1.0)) }
	pub fn yellow()   -> Self { Color(Vec4::new(1.0, 1.0, 0.0, 1.0)) }
	pub fn blue()     -> Self { Color(Vec4::new(0.0, 0.0, 1.0, 1.0)) }
	pub fn magenta()  -> Self { Color(Vec4::new(1.0, 0.0, 1.0, 1.0)) }
	pub fn cyan()     -> Self { Color(Vec4::new(0.0, 1.0, 1.0, 1.0)) }
	pub fn white()    -> Self { Color(Vec4::new(1.0, 1.0, 1.0, 1.0)) }
	
	pub fn opactiy(self, opacity: f32) -> Self { Color(self.0 * opacity) }
	pub fn lightness(self, lightness: f32) -> Self { Color(self.0.component_mul(&Vec4::new(lightness, lightness, lightness, 1.0))) }
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

