use fast_image_resize::{PixelType, ResizeOptions, Resizer, images::Image};
use image::DynamicImage;

use super::PreviewError;

pub fn image_size(image: &DynamicImage) -> (u32, u32) {
    (image.width(), image.height())
}

pub fn rgb(image: DynamicImage) -> Result<Image<'static>, PreviewError> {
    let size = image_size(&image);
    pixels(image.into_rgb8().into_raw(), size, PixelType::U8x3)
}

pub fn pixels(
    buffer: Vec<u8>,
    (width, height): (u32, u32),
    pixel_type: PixelType,
) -> Result<Image<'static>, PreviewError> {
    Image::from_vec_u8(width, height, buffer, pixel_type)
        .map_err(|e| PreviewError::Decode(e.to_string()))
}

/// `image` at `width`×`height`, or as it is when it's that size already.
pub fn resize(
    resizer: &mut Resizer,
    image: Image<'static>,
    width: u32,
    height: u32,
) -> Result<Image<'static>, PreviewError> {
    if (width, height) == (image.width(), image.height()) {
        return Ok(image);
    }
    let mut resized = Image::new(width, height, image.pixel_type());
    resizer
        .resize(&image, &mut resized, &ResizeOptions::new())
        .map_err(|e| PreviewError::Encode(e.to_string()))?;
    Ok(resized)
}

/// `width`×`height` scaled down so the longest side is at most `max_side`.
pub fn fit(width: u32, height: u32, max_side: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_side {
        return (width, height);
    }
    let scale = |side: u32| {
        let scaled =
            (u64::from(side) * u64::from(max_side) + u64::from(longest) / 2) / u64::from(longest);
        // At most `max_side`, so it fits.
        (scaled as u32).max(1)
    };
    (scale(width), scale(height))
}

/// `side` of one image scaled like `from` became `to` in another.
pub fn proportional(to: u32, side: u32, from: u32) -> u32 {
    let scaled = (u64::from(to) * u64::from(side) + u64::from(from) / 2) / u64::from(from);
    // At most `side`, as `to` is at most `from`.
    (scaled as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_longest_side() {
        assert_eq!(fit(4032, 3024, 512), (512, 384));
        assert_eq!(fit(2520, 4032, 256), (160, 256));
        assert_eq!(fit(10000, 1, 128), (128, 1));
    }

    #[test]
    fn never_upscales() {
        assert_eq!(fit(100, 50, 512), (100, 50));
        assert_eq!(fit(512, 512, 512), (512, 512));
    }
}
