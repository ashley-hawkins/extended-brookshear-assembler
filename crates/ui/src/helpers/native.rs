use super::{DisplayImageReceiver, FileReceiver, RgbaImage};

pub fn open_file() -> FileReceiver {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let path = rfd::FileDialog::new().pick_file();

    if let Some(path) = path {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let file_contents = std::fs::read(&path)
            .map_err(|err| format!("Failed to read '{}': {err}", path.display()));
        let _ = sender.send(file_contents.map(|contents| (file_name, contents)));
    } else {
        drop(sender);
    }
    receiver
}

pub fn save_file(data: Vec<u8>, name_hint: &str) -> Result<(), String> {
    let path = rfd::FileDialog::new().set_file_name(name_hint).save_file();
    if let Some(path) = path {
        std::fs::write(&path, data)
            .map_err(|err| format!("Failed to write '{}': {err}", path.display()))?;
    }
    Ok(())
}

pub fn save_rgba_image(image: RgbaImage, name_hint: &str) -> Result<(), String> {
    let mut png_data = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut png_data),
            image::ImageFormat::Png,
        )
        .map_err(|err| format!("Failed to encode image as PNG: {err}"))?;
    save_file(png_data, name_hint)
}

pub fn decode_rgba_image_async(
    bytes: Vec<u8>,
    decode: impl FnOnce(&RgbaImage) -> super::DisplayImageResult,
) -> DisplayImageReceiver {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let result = decode_rgba_image(&bytes).and_then(|image| decode(&image));
    let _ = sender.send(result);
    receiver
}

fn decode_rgba_image(data: &[u8]) -> Result<RgbaImage, String> {
    use image::ImageReader;

    let image = ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|err| format!("Failed to identify image format: {err}"))?
        .decode()
        .map_err(|err| format!("Failed to decode image: {err}"))?
        .to_rgba8();

    Ok(image)
}
