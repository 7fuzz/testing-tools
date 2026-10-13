/// RGBA (alpha ignored) to YUYV 4:2:2 with BT.601 limited-range coefficients, two pixels per 4 bytes.
/// An odd last column repeats its pixel.
pub fn rgba_to_yuyv(rgba: &[u8], width: u32, height: u32, out: &mut Vec<u8>) {
    let (width, height) = (width as usize, height as usize);
    out.clear();
    out.reserve(width.div_ceil(2) * 4 * height);
    for row in rgba.chunks_exact(width * 4).take(height) {
        for pair in row.chunks(8) {
            let (r0, g0, b0) = (i32::from(pair[0]), i32::from(pair[1]), i32::from(pair[2]));
            let (r1, g1, b1) = if pair.len() >= 8 { (i32::from(pair[4]), i32::from(pair[5]), i32::from(pair[6])) } else { (r0, g0, b0) };
            let y0 = ((66 * r0 + 129 * g0 + 25 * b0 + 128) >> 8) + 16;
            let y1 = ((66 * r1 + 129 * g1 + 25 * b1 + 128) >> 8) + 16;
            let (r, g, b) = ((r0 + r1) >> 1, (g0 + g1) >> 1, (b0 + b1) >> 1);
            let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
            let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
            out.extend_from_slice(&[y0.clamp(16, 235) as u8, u.clamp(16, 240) as u8, y1.clamp(16, 235) as u8, v.clamp(16, 240) as u8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_known_colours() {
        let mut out = Vec::new();
        rgba_to_yuyv(&vec![0; 640 * 480 * 4], 640, 480, &mut out);
        assert_eq!(out.len(), 640 * 480 * 2);
        assert_eq!(&out[..4], &[16, 128, 16, 128]);

        rgba_to_yuyv(&[255, 255, 255, 255, 255, 255, 255, 255], 2, 1, &mut out);
        assert_eq!(out, [235, 128, 235, 128]);
    }
}
