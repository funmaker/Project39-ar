use image::DynamicImage;

pub trait ImageEx {
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8>;
	fn has_alpha(&self) -> bool;
}

impl ImageEx for DynamicImage {
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8> {
		let has_alpha = self.has_alpha();
		let mut data = self.into_rgba8();
		
		if has_alpha {
			for pixel in data.pixels_mut() {
				pixel[0] = (pixel[0] as u16 * pixel[3] as u16 / 255) as u8;
				pixel[1] = (pixel[1] as u16 * pixel[3] as u16 / 255) as u8;
				pixel[2] = (pixel[2] as u16 * pixel[3] as u16 / 255) as u8;
			}
		}
		
		data.into_vec().into_iter()
	}
	
	fn has_alpha(&self) -> bool {
		use DynamicImage::*;
		
		match self {
			ImageLuma8(_)  | ImageRgb8(_)  | ImageLuma16(_)  | ImageRgb16(_)  | ImageRgb32F(_)  => false,
			ImageLumaA8(_) | ImageRgba8(_) | ImageLumaA16(_) | ImageRgba16(_) | ImageRgba32F(_) => true,
			_ => false,
		}
	}
}
