// Pure-Rust PNG encoder (no external dependencies).
//
// Produces valid PNG files with RGBA colour (colour type 6) using
// zlib-wrapped stored (type-00) DEFLATE blocks — i.e. uncompressed.
// The output is bit-for-bit identical to what a proper encoder would
// produce for the same pixels, just larger than a compressed version.

// ── CRC-32 ──────────────────────────────────────────────────────────────────

fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for n in 0u32..256 {
        let mut c = n;
        for _ in 0..8 {
            if c & 1 != 0 {
                c = 0xEDB88320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
        }
        table[n as usize] = c;
    }
    table
}

fn crc32(data: &[u8]) -> u32 {
    let table = make_crc_table();
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

// ── Adler-32 ────────────────────────────────────────────────────────────────

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % MOD;
        s2 = (s2 + s1) % MOD;
    }
    (s2 << 16) | s1
}

// ── zlib (stored-block DEFLATE) ──────────────────────────────────────────────

/// Wrap `data` in a zlib stream using uncompressed (type 00) DEFLATE blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    // zlib header: CMF=0x78, FLG chosen so that CMF*256+FLG is divisible by 31.
    // 0x78 * 256 + 0x01 = 30721; 30721 % 31 = 0. ✓
    let mut out = Vec::new();
    out.push(0x78);
    out.push(0x01);

    // Emit stored DEFLATE blocks of at most 65535 bytes each.
    let mut pos = 0;
    while pos < data.len() || data.is_empty() {
        let end = (pos + 65535).min(data.len());
        let block = &data[pos..end];
        let is_last = end == data.len();
        let len = block.len() as u16;
        let nlen = !len;

        out.push(if is_last { 0x01 } else { 0x00 }); // BFINAL | BTYPE=00
        out.push((len & 0xFF) as u8);
        out.push((len >> 8) as u8);
        out.push((nlen & 0xFF) as u8);
        out.push((nlen >> 8) as u8);
        out.extend_from_slice(block);

        if is_last {
            break;
        }
        pos = end;
    }

    // Adler-32 of the original data (big-endian).
    let a = adler32(data);
    out.push((a >> 24) as u8);
    out.push((a >> 16) as u8);
    out.push((a >> 8) as u8);
    out.push((a & 0xFF) as u8);

    out
}

// ── PNG chunk helpers ────────────────────────────────────────────────────────

fn png_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let len = data.len() as u32;
    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&len.to_be_bytes());
    chunk.extend_from_slice(tag);
    chunk.extend_from_slice(data);
    // CRC covers type + data.
    let crc_input: Vec<u8> = tag.iter().chain(data).copied().collect();
    chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    chunk
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Encode an RGBA pixel buffer as a PNG file.
///
/// `pixels` must be in row-major order, 4 bytes per pixel (R, G, B, A).
/// The length must equal `width * height * 4`.
pub fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    assert_eq!(pixels.len() as u64, width as u64 * height as u64 * 4);

    // 1. Build raw filter-byte + scanline data.
    let bpp = 4usize; // bytes per pixel (RGBA)
    let stride = width as usize * bpp;
    let mut raw = Vec::with_capacity((1 + stride) * height as usize);
    for row in 0..height as usize {
        raw.push(0); // filter type 0 = None
        raw.extend_from_slice(&pixels[row * stride..(row + 1) * stride]);
    }

    // 2. Compress with zlib (stored).
    let compressed = zlib_stored(&raw);

    // 3. Assemble PNG.
    let signature: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // colour type: RGBA
    ihdr.push(0); // compression method
    ihdr.push(0); // filter method
    ihdr.push(0); // interlace method

    let mut out = Vec::new();
    out.extend_from_slice(&signature);
    out.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    out.extend_from_slice(&png_chunk(b"IDAT", &compressed));
    out.extend_from_slice(&png_chunk(b"IEND", &[]));
    out
}

// ── Base-64 encoder ──────────────────────────────────────────────────────────

const B64_ALPHABET: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode `data` to standard base-64 (with `=` padding).
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut chunks = data.chunks_exact(3);
    for chunk in chunks.by_ref() {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[(n & 0x3F) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn test_base64_known() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn test_crc32_known() {
        // CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_adler32_known() {
        // adler32("Wikipedia") = 0x11E60398 (standard example)
        assert_eq!(adler32(b"Wikipedia"), 0x11E60398);
    }

    #[test]
    fn test_png_header() {
        // A 1×1 white pixel.
        let pixels = [255u8, 255, 255, 255];
        let png = encode_png(1, 1, &pixels);
        // PNG signature
        assert_eq!(&png[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        // IHDR tag at offset 12
        assert_eq!(&png[12..16], b"IHDR");
    }
}
