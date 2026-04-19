#[cfg(target_arch = "wasm32")]
use js_sys::wasm_bindgen::{self, prelude::wasm_bindgen};

#[cfg(not(target_arch = "wasm32"))]
pub fn open_file() -> futures::channel::oneshot::Receiver<(String, Vec<u8>)> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let path = rfd::FileDialog::new().pick_file();

    if let Some(path) = path {
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        sender
            .send((file_name, std::fs::read(path).expect("Failed to read file")))
            .expect("Failed to send path through channel");
    } else {
        drop(sender);
    }
    receiver
}

#[cfg(target_arch = "wasm32")]
pub fn open_file() -> futures::channel::oneshot::Receiver<(String, Vec<u8>)> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let file = rfd::AsyncFileDialog::new().pick_file().await;
        if let Some(file) = file {
            let data = file.read().await;
            sender
                .send((file.file_name(), data))
                .expect("Failed to send file data through channel");
        } else {
            drop(sender);
        }
    });
    receiver
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(data: Vec<u8>, name_hint: &str) {
    let path = rfd::FileDialog::new().set_file_name(name_hint).save_file();
    if let Some(path) = path {
        std::fs::write(path, data).expect("Failed to write file");
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(data: Vec<u8>, name_hint: &str) {
    use js_sys::wasm_bindgen::JsCast;
    let blob = web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(
        &js_sys::Uint8Array::from(&data[..]),
    ))
    .expect("Failed to create blob");
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).expect("Failed to create object URL");
    let a = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("a")
        .unwrap()
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .unwrap();
    a.set_attribute("href", &url).unwrap();
    a.set_attribute("download", name_hint).unwrap();
    a.click();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render_and_save_image(bytes: &[u8; 128], name_hint: &str) {
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
    .expect("Failed to write image to PNG format");
    save_file(png_data, name_hint);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function render_and_save_image(bytes, nameHint) {
    const canvas = document.createElement('canvas');
    canvas.width = 32;
    canvas.height = 32;
    const ctx = canvas.getContext('2d');
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
    canvas.toBlob(blob => {
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = nameHint;
        a.click();
        URL.revokeObjectURL(url);
    });
}
"#)]
extern "C" {
    pub fn render_and_save_image(bytes: &[u8], name_hint: &str);
}
