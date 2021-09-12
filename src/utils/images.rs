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
		match self {
			DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgb16(_) | DynamicImage::ImageBgr8(_) | DynamicImage::ImageLuma16(_) => false,
			DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_) | DynamicImage::ImageRgba16(_) | DynamicImage::ImageBgra8(_)| DynamicImage::ImageLumaA16(_) => true,
		}
	}
}
