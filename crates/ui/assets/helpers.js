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
