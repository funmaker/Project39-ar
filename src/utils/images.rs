use image::DynamicImage;


/// Helpers for converting images into RGBA8 texture data with premultiplied alpha.
pub trait DynamicImageEx {
	/// Returns RGBA8 bytes with alpha premultiplied directly on the stored values (correct for linear images).
	fn into_lin_pre_mul_iter(self) -> std::vec::IntoIter<u8>;
	/// Returns RGBA8 bytes with alpha premultiplied in linear space, for sRGB images.
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8>;
	/// Returns `true` if the image format has an alpha channel.
	fn has_alpha(&self) -> bool;
}

impl DynamicImageEx for DynamicImage {
	fn into_lin_pre_mul_iter(self) -> std::vec::IntoIter<u8> {
		let has_alpha = self.has_alpha();
		let mut data = self.into_rgba8();
		
		if has_alpha {
			for pixel in data.pixels_mut() {
				if pixel[3] < 255 {
					pixel[0] = (pixel[0] as u16 * pixel[3] as u16 / 255) as u8;
					pixel[1] = (pixel[1] as u16 * pixel[3] as u16 / 255) as u8;
					pixel[2] = (pixel[2] as u16 * pixel[3] as u16 / 255) as u8;
				}
			}
		}
		
		data.into_vec().into_iter()
	}
	
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8> {
		let has_alpha = self.has_alpha();
		let mut data = self.into_rgba8();
		
		if has_alpha {
			for pixel in data.pixels_mut() {
				if pixel[3] < 255 {
					pixel[0] = to_srgb(to_linear(pixel[0]) * pixel[3] as f32 / 255.0);
					pixel[1] = to_srgb(to_linear(pixel[1]) * pixel[3] as f32 / 255.0);
					pixel[2] = to_srgb(to_linear(pixel[2]) * pixel[3] as f32 / 255.0);
				}
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

/// Converts an sRGB encoded byte to a linear value in 0..1.
fn to_linear(srgb: u8) -> f32 {
	if srgb <= (0.0031308 / 12.92 * 255.0) as u8 {
		srgb as f32 / 255.0 / 12.92
	} else {
		f32::powf((srgb as f32 / 255.0 + 0.055) / 1.055, 2.4)
	}
}

/// Converts a linear value in 0..1 to an sRGB encoded byte.
fn to_srgb(linear: f32) -> u8 {
	if linear <= 0.0031308 {
		(linear * 12.92 * 255.0) as u8
	} else {
		((1.055 * linear.powf(1.0 / 2.4) - 0.055) * 255.0) as u8
	}
}
