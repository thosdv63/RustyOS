// BMP coz/olustur (24/32-bit sikistirmasiz)
use alloc::vec::Vec;
use alloc::vec;

pub fn decode(d: &[u8]) -> Option<(usize, usize, Vec<u32>)> {
    if d.len() < 54 || d[0] != b'B' || d[1] != b'M' { return None; }
    let off = u32::from_le_bytes([d[10], d[11], d[12], d[13]]) as usize;
    let w = i32::from_le_bytes([d[18], d[19], d[20], d[21]]);
    let h_raw = i32::from_le_bytes([d[22], d[23], d[24], d[25]]);
    let bpp = u16::from_le_bytes([d[28], d[29]]) as usize;
    let comp = u32::from_le_bytes([d[30], d[31], d[32], d[33]]);
    if comp != 0 && comp != 3 { return None; }
    if bpp != 24 && bpp != 32 { return None; }
    if w <= 0 || w > 4096 || h_raw == 0 || h_raw.abs() > 4096 { return None; }

    let w = w as usize;
    let top_down = h_raw < 0;
    let h = h_raw.unsigned_abs() as usize;
    let bypp = bpp / 8;
    let stride = (w * bypp + 3) & !3;
    if off + stride * h > d.len() { return None; }

    let mut px = vec![0u32; w * h];
    for row in 0..h {
        let src_row = if top_down { row } else { h - 1 - row };
        let ro = off + src_row * stride;
        for col in 0..w {
            let p = ro + col * bypp;
            let (b, g, r) = (d[p] as u32, d[p+1] as u32, d[p+2] as u32);
            px[row * w + col] = (r << 16) | (g << 8) | b;
        }
    }
    Some((w, h, px))
}

pub fn encode24(w: usize, h: usize, px: &[u32]) -> Vec<u8> {
    let stride = (w * 3 + 3) & !3;
    let dsize = stride * h;
    let mut out = vec![0u8; 54 + dsize];
    out[0] = b'B'; out[1] = b'M';
    out[2..6].copy_from_slice(&((54 + dsize) as u32).to_le_bytes());
    out[10..14].copy_from_slice(&54u32.to_le_bytes());
    out[14..18].copy_from_slice(&40u32.to_le_bytes());
    out[18..22].copy_from_slice(&(w as u32).to_le_bytes());
    out[22..26].copy_from_slice(&(h as u32).to_le_bytes());
    out[26..28].copy_from_slice(&1u16.to_le_bytes());
    out[28..30].copy_from_slice(&24u16.to_le_bytes());
    out[34..38].copy_from_slice(&(dsize as u32).to_le_bytes());
    for row in 0..h {
        let ro = 54 + (h - 1 - row) * stride;
        for col in 0..w {
            let c = px[row * w + col];
            let p = ro + col * 3;
            out[p] = c as u8; out[p+1] = (c >> 8) as u8; out[p+2] = (c >> 16) as u8;
        }
    }
    out
}