use super::{DisplayImageReceiver, FileReceiver, RgbaImage};
use js_sys::wasm_bindgen::{self, prelude::wasm_bindgen};

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

pub fn save_file(data: Vec<u8>, name_hint: &str) -> Result<(), String> {
    save_file_js(&data, name_hint).map_err(js_value_to_string)
}

pub fn save_rgba_image(image: RgbaImage, name_hint: &str) -> Result<(), String> {
    let width = image.width();
    let height = image.height();
    let rgba = image.into_raw();
    save_rgba_image_js(&rgba, width, height, name_hint).map_err(js_value_to_string)
}

pub fn decode_rgba_image_async(
    bytes: Vec<u8>,
    decode: impl FnOnce(&RgbaImage) -> super::DisplayImageResult + 'static,
) -> DisplayImageReceiver {
    let (sender, receiver) = futures::channel::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let result = match decode_rgba_image(&bytes).await {
            Ok(image) => decode(&image),
            Err(err) => Err(err),
        };
        let _ = sender.send(result);
    });
    receiver
}

async fn decode_rgba_image(data: &[u8]) -> Result<RgbaImage, String> {
    use wasm_bindgen::JsCast;

    let canvas = decode_image_to_canvas_js(data)
        .await
        .map_err(js_value_to_string)?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|err| format!("Failed to convert decoded image into canvas: {err:?}"))?;
    let context = canvas
        .get_context("2d")
        .map_err(|err| format!("Failed to get 2D rendering context: {err:?}"))?
        .ok_or_else(|| "Failed to get 2D rendering context".to_string())?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|err| format!("Failed to convert 2D context: {err:?}"))?;
    let data = context
        .get_image_data(0.0, 0.0, canvas.width() as f64, canvas.height() as f64)
        .map_err(|err| format!("Failed to read decoded image pixels: {err:?}"))?
        .data()
        .0;

    RgbaImage::from_raw(canvas.width(), canvas.height(), data)
        .ok_or_else(|| "Failed to construct RGBA image from web pixel data".to_string())
}

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

export function save_rgba_image(rgba, width, height, nameHint) {
    const document = ensureDocument();
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
        throw new Error('Failed to get 2D rendering context');
    }
    const imageData = new ImageData(new Uint8ClampedArray(rgba), width, height);
    ctx.putImageData(imageData, 0, 0);
    const dataUrl = canvas.toDataURL('image/png');
    triggerDownload(dataUrl, nameHint);
}

export async function decode_image_to_canvas(bytes) {
    const document = ensureDocument();
    const blob = new Blob([bytes]);
    const imageBitmap = await createImageBitmap(blob);
    try {
        const canvas = document.createElement('canvas');
        canvas.width = imageBitmap.width;
        canvas.height = imageBitmap.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) {
            throw new Error('Failed to get 2D rendering context');
        }
        ctx.drawImage(imageBitmap, 0, 0);
        return canvas;
    } finally {
        imageBitmap.close();
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = save_file)]
    fn save_file_js(bytes: &[u8], name_hint: &str) -> Result<(), wasm_bindgen::JsValue>;

    #[wasm_bindgen(catch, js_name = save_rgba_image)]
    fn save_rgba_image_js(
        rgba: &[u8],
        width: u32,
        height: u32,
        name_hint: &str,
    ) -> Result<(), wasm_bindgen::JsValue>;

    #[wasm_bindgen(catch, js_name = decode_image_to_canvas)]
    async fn decode_image_to_canvas_js(
        bytes: &[u8],
    ) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
}

fn js_value_to_string(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {value:?}"))
}
