use fast_image_resize::{PixelType, images::Image};
use jxl_oxide::{EnumColourEncoding, FrameBufferSample, ImageStream, JxlImage, RenderingIntent};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct DecodeError(String);

impl From<Box<dyn std::error::Error + Send + Sync>> for DecodeError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self(e.to_string())
    }
}

/// The color encoding to decode into.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// 8-bit sRGB: `U8x3`, or `U8x4` with alpha.
    Srgb,
    /// 16-bit BT.2100 PQ: `U16x3`, or `U16x4` with alpha.
    Pq,
}

/// The first frame of the JPEG XL `file`, upright, as RGB in `output`'s
/// encoding. Grayscale and CMYK come out as RGB too.
pub fn decode(file: &[u8], output: Output) -> Result<Image<'static>, DecodeError> {
    let mut image = JxlImage::builder().read(file)?;
    image.request_color_encoding(match output {
        Output::Srgb => EnumColourEncoding::srgb(RenderingIntent::Relative),
        Output::Pq => EnumColourEncoding::bt2100_pq(RenderingIntent::Relative),
    });
    if image.num_loaded_keyframes() == 0 {
        return Err(DecodeError("JPEG XL without a frame".into()));
    }
    let render = image.render_frame(0)?;
    let stream = render.stream();
    let (width, height) = (stream.width(), stream.height());
    let (bytes, pixel_type) = match output {
        Output::Srgb => rgb::<u8>(stream, [PixelType::U8x3, PixelType::U8x4])?,
        Output::Pq => {
            let (samples, pixel_type) = rgb::<u16>(stream, [PixelType::U16x3, PixelType::U16x4])?;
            (
                samples.iter().flat_map(|v| v.to_ne_bytes()).collect(),
                pixel_type,
            )
        }
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
