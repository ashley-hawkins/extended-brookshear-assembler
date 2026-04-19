#[cfg(target_arch = "wasm32")]
use js_sys::wasm_bindgen::{self, prelude::wasm_bindgen};

pub type NamedFile = (String, Vec<u8>);
pub type FileResult = Result<NamedFile, String>;
pub type FileReceiver = futures::channel::oneshot::Receiver<FileResult>;

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
pub fn open_file() -> FileReceiver {
    let (sender, receiver) = futures::channel::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let file = rfd::AsyncFileDialog::new().pick_file().await;
        if let Some(file) = file {
            let data = file.read().await;
            let _ = sender.send(Ok((file.file_name(), data)));
        } else {
            drop(sender);
        }
    });
    receiver
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(data: Vec<u8>, name_hint: &str) -> Result<(), String> {
    let path = rfd::FileDialog::new().set_file_name(name_hint).save_file();
    if let Some(path) = path {
        std::fs::write(&path, data)
            .map_err(|err| format!("Failed to write '{}': {err}", path.display()))?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(data: Vec<u8>, name_hint: &str) -> Result<(), String> {
    save_file_js(&data, name_hint).map_err(js_value_to_string)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render_and_save_image(bytes: &[u8; 128], name_hint: &str) -> Result<(), String> {
    use image::{ImageBuffer, Rgba};
    let img = ImageBuffer::<Rgba<u8>, _>::from_fn(32, 32, |col, row| {
        let address = (row * (32 / 8) + col / 8) as usize;
        let byte = bytes[address];
        let bit = byte & (1 << (7 - (col % 8)));
        let color = if bit != 0 { 255 } else { 0 };
        image::Rgba([color, color, color, 255])
    });

    let mut png_data = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_data),
        image::ImageFormat::Png,
    )
    .map_err(|err| format!("Failed to encode image as PNG: {err}"))?;
    save_file(png_data, name_hint)
}

#[cfg(target_arch = "wasm32")]
pub fn render_and_save_image(bytes: &[u8], name_hint: &str) -> Result<(), String> {
    render_and_save_image_js(bytes, name_hint).map_err(js_value_to_string)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
function ensureDocument() {
    const document = globalThis.document;
    if (!document) {
        throw new Error('No document available');
    }
    return document;
}

function triggerDownload(url, nameHint) {
    const document = ensureDocument();
    const a = document.createElement('a');
    a.href = url;
    a.download = nameHint;
    a.click();
}

export function save_file(bytes, nameHint) {
    const blob = new Blob([bytes], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    try {
        triggerDownload(url, nameHint);
    } finally {
        URL.revokeObjectURL(url);
    }
}

export function render_and_save_image(bytes, nameHint) {
    const document = ensureDocument();
    const canvas = document.createElement('canvas');
    canvas.width = 32;
    canvas.height = 32;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
        throw new Error('Failed to get 2D rendering context');
    }
    const imageData = ctx.createImageData(32, 32);
    for (let row = 0; row < 32; row++) {
        for (let col = 0; col < 32; col++) {
            let address = (row * (32 / 8) + col / 8) | 0;
            let byte = bytes[address];
            let bit = byte & (1 << (7 - (col % 8)));
            let color = bit ? 255 : 0;
            let index = (row * 32 + col) * 4;
            imageData.data[index] = color; // R
            imageData.data[index + 1] = color; // G
            imageData.data[index + 2] = color; // B
            imageData.data[index + 3] = 255; // A
        }
    }
    ctx.putImageData(imageData, 0, 0);
    const dataUrl = canvas.toDataURL('image/png');
    triggerDownload(dataUrl, nameHint);
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = save_file)]
    fn save_file_js(bytes: &[u8], name_hint: &str) -> Result<(), wasm_bindgen::JsValue>;

    #[wasm_bindgen(catch, js_name = render_and_save_image)]
    fn render_and_save_image_js(
        bytes: &[u8],
        name_hint: &str,
    ) -> Result<(), wasm_bindgen::JsValue>;
}

#[cfg(target_arch = "wasm32")]
fn js_value_to_string(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {value:?}"))
}
