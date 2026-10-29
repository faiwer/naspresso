use fast_image_resize::{PixelType, images::Image};
use jxl_oxide::{
    EnumColourEncoding, FrameBufferSample, ImageStream, JxlImage, PixelFormat, RenderingIntent,
};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct DecodeError(String);

impl DecodeError {
    pub fn new(message: &str) -> Self {
        Self(message.into())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for DecodeError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self(e.to_string())
    }
}

/// The first frame of the JPEG XL `file`, upright, as 16-bit BT.2100 PQ RGB:
/// `U16x3`, or `U16x4` with alpha. Grayscale and CMYK come out as RGB too.
pub fn decode_pq(file: &[u8]) -> Result<Image<'static>, DecodeError> {
    let mut image = JxlImage::builder().read(file)?;
    image.request_color_encoding(EnumColourEncoding::bt2100_pq(RenderingIntent::Relative));
    render_first_frame(&image, true)
}

/// The first frame as 8-bit RGB, `U8x3` or `U8x4`, upright. An RGB image
/// stays in its own color space, which the returned ICC profile describes;
/// gray and CMYK images become sRGB, without a profile.
pub fn decode_sdr(file: &[u8]) -> Result<(Image<'static>, Option<Vec<u8>>), DecodeError> {
    let mut image = JxlImage::builder().read(file)?;
    if matches!(image.pixel_format(), PixelFormat::Rgb | PixelFormat::Rgba) {
        return Ok((
            render_first_frame(&image, false)?,
            Some(image.rendered_icc()),
        ));
    }
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    Ok((render_first_frame(&image, false)?, None))
}

/// Like [`decode_sdr`], but gray and CMYK images keep their own color space
/// too, so the profile may not be an RGB one.
pub fn decode_native(file: &[u8]) -> Result<(Image<'static>, Vec<u8>), DecodeError> {
    let image = JxlImage::builder().read(file)?;
    Ok((render_first_frame(&image, false)?, image.rendered_icc()))
}

/// The first frame as 8-bit or, if `wide`, 16-bit RGB.
fn render_first_frame(image: &JxlImage, wide: bool) -> Result<Image<'static>, DecodeError> {
    if image.num_loaded_keyframes() == 0 {
        return Err(DecodeError::new("JPEG XL without a frame"));
    }
    let render = image.render_frame(0)?;
    let stream = render.stream();
    let (width, height) = (stream.width(), stream.height());
    let (bytes, pixel_type) = if wide {
        let (samples, pixel_type) = rgb::<u16>(stream, [PixelType::U16x3, PixelType::U16x4])?;
        (
            samples.iter().flat_map(|v| v.to_ne_bytes()).collect(),
            pixel_type,
        )
    } else {
        rgb::<u8>(stream, [PixelType::U8x3, PixelType::U8x4])?
    };
    Image::from_vec_u8(width, height, bytes, pixel_type).map_err(|e| DecodeError(e.to_string()))
}

/// The JPEG packed into the JPEG XL `file`, byte for byte, with whatever
/// followed the image, like a gain map.
pub fn unpack_jpeg(file: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let image = JxlImage::builder().read(file)?;
    let mut jpeg = Vec::new();
    image.reconstruct_jpeg(&mut jpeg)?;
    Ok(jpeg)
}

/// Every sample of `stream` with gray widened to RGB, and the pixel type out
/// of `[rgb, rgba]`.
fn rgb<S: FrameBufferSample + Copy>(
    mut stream: ImageStream,
    [rgb, rgba]: [PixelType; 2],
) -> Result<(Vec<S>, PixelType), DecodeError> {
    let channels = stream.channels() as usize;
    let len = stream.width() as usize * stream.height() as usize * channels;
    let mut samples = vec![S::default(); len];
    stream.write_to_buffer(&mut samples);
    Ok(match channels {
        1 => (samples.iter().flat_map(|&v| [v; 3]).collect(), rgb),
        2 => (
            samples
                .chunks_exact(2)
                .flat_map(|px| [px[0], px[0], px[0], px[1]])
                .collect(),
            rgba,
        ),
        3 => (samples, rgb),
        4 => (samples, rgba),
        other => return Err(DecodeError(format!("JPEG XL with {other} channels"))),
    })
}
