#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

use image::RgbaImage;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(target_arch = "wasm32")]
use web as platform;

pub type NamedFile = (String, Vec<u8>);
pub type FileResult = Result<NamedFile, String>;
pub type FileReceiver = futures::channel::oneshot::Receiver<FileResult>;
pub type DisplayImageResult = Result<[u8; 128], String>;
pub type DisplayImageReceiver = futures::channel::oneshot::Receiver<DisplayImageResult>;

const DISPLAY_WIDTH: u32 = 32;
const DISPLAY_HEIGHT: u32 = 32;
const DISPLAY_STRIDE_BYTES: u32 = DISPLAY_WIDTH / 8;
const PIXEL_OFF: image::Rgba<u8> = image::Rgba([0, 0, 0, 255]);
const PIXEL_ON: image::Rgba<u8> = image::Rgba([255, 255, 255, 255]);

pub use platform::{open_file, save_file};

fn display_memory_to_rgba_image(bytes: &[u8; 128]) -> RgbaImage {
    let mut image = RgbaImage::new(DISPLAY_WIDTH, DISPLAY_HEIGHT);

    for row in 0..DISPLAY_HEIGHT {
        for col in 0..DISPLAY_WIDTH {
            let address = (row * DISPLAY_STRIDE_BYTES + col / 8) as usize;
            let byte = bytes[address];
            let bit = byte & (1 << (7 - (col % 8)));
            image.put_pixel(col, row, if bit != 0 { PIXEL_ON } else { PIXEL_OFF });
        }
    }

    image
}

fn rgba_image_to_display_memory(image: &RgbaImage) -> Result<[u8; 128], String> {
    if image.width() != DISPLAY_WIDTH || image.height() != DISPLAY_HEIGHT {
        return Err(format!(
            "Display images must be exactly 32x32 pixels, but this image is {}x{}.",
            image.width(),
            image.height()
        ));
    }

    let mut output = [0u8; 128];
    let first_value = pixel_value(image.get_pixel(0, 0));
    let mut second_value = None;

    'outer: for row in 0..DISPLAY_HEIGHT {
        for col in 0..DISPLAY_WIDTH {
            let value = pixel_value(image.get_pixel(col, row));
            if value != first_value {
                second_value = Some(value);
                break 'outer;
            }
        }
    }

    let Some(second_value) = second_value else {
        return Err(
            "Display image must contain exactly two colors so the on/off pixels can be determined."
                .to_string(),
        );
    };

    let on_value = first_value.max(second_value);
    let off_value = first_value.min(second_value);

    for row in 0..DISPLAY_HEIGHT {
        for col in 0..DISPLAY_WIDTH {
            let value = pixel_value(image.get_pixel(col, row));
            let bit = if value == on_value {
                1
            } else if value == off_value {
                0
            } else {
                return Err(format!(
                    "Encountered a third pixel value ({value}) at ({col}, {row}); only two colors are supported."
                ));
            };

            if bit != 0 {
                let address = (row * DISPLAY_STRIDE_BYTES + col / 8) as usize;
                output[address] |= 1 << (7 - (col % 8));
            }
        }
    }

    Ok(output)
}

fn pixel_value(pixel: &image::Rgba<u8>) -> u8 {
    *pixel.0[..3].iter().max().unwrap()
}

pub fn render_and_save_image(bytes: &[u8; 128], name_hint: &str) -> Result<(), String> {
    platform::save_rgba_image(display_memory_to_rgba_image(bytes), name_hint)
}

pub fn decode_display_image_async(bytes: Vec<u8>) -> DisplayImageReceiver {
    platform::decode_rgba_image_async(bytes, rgba_image_to_display_memory)
}
