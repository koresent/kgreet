// Three-pass box blur approximates a Gaussian blur in O(W * H) time.
pub fn blur_rgba(pixels: &mut [u8], width: usize, height: usize, sigma: f32) {
    if sigma <= 0.1 || width < 2 || height < 2 || pixels.len() < width * height * 4 {
        return;
    }

    let boxes = boxes_for_gauss(sigma, 3);
    let mut scratch = pixels.to_vec();

    for radius in boxes {
        if radius == 0 {
            continue;
        }
        box_blur_h(pixels, &mut scratch, width, height, radius);
        box_blur_v(&scratch, pixels, width, height, radius);
    }
}

fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<usize> {
    let w_ideal = ((12.0 * sigma * sigma / n as f32) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as usize;
    if wl.is_multiple_of(2) {
        wl = wl.saturating_sub(1);
    }
    let wu = wl + 2;

    let m_ideal = (12.0 * sigma * sigma
        - (n * wl * wl) as f32
        - (4 * n * wl) as f32
        - (3 * n) as f32)
        / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round().clamp(0.0, n as f32) as usize;

    (0..n)
        .map(|i| {
            let size = if i < m { wl } else { wu };
            (size.saturating_sub(1)) / 2
        })
        .collect()
}

fn box_blur_h(src: &[u8], dst: &mut [u8], width: usize, height: usize, r: usize) {
    let iarr = 1.0 / (r + r + 1) as f32;
    let max_x = width - 1;

    for y in 0..height {
        let row = y * width * 4;
        let first = row;
        let last = row + max_x * 4;

        let mut acc = [0i32; 4];
        for ch in 0..4 {
            acc[ch] = src[first + ch] as i32 * (r as i32 + 1);
        }
        for x in 0..r {
            let idx = row + x.min(max_x) * 4;
            for ch in 0..4 {
                acc[ch] += src[idx + ch] as i32;
            }
        }

        for x in 0..width {
            let right_idx = if x + r <= max_x {
                row + (x + r) * 4
            } else {
                last
            };
            let left_idx = if x > r { row + (x - r - 1) * 4 } else { first };
            let out_idx = row + x * 4;

            for ch in 0..4 {
                acc[ch] += src[right_idx + ch] as i32 - src[left_idx + ch] as i32;
                dst[out_idx + ch] = ((acc[ch] as f32 * iarr).round()).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], width: usize, height: usize, r: usize) {
    let iarr = 1.0 / (r + r + 1) as f32;
    let stride = width * 4;
    let max_y = height - 1;

    for x in 0..width {
        let col = x * 4;
        let first = col;
        let last = max_y * stride + col;

        let mut acc = [0i32; 4];
        for ch in 0..4 {
            acc[ch] = src[first + ch] as i32 * (r as i32 + 1);
        }
        for y in 0..r {
            let idx = y.min(max_y) * stride + col;
            for ch in 0..4 {
                acc[ch] += src[idx + ch] as i32;
            }
        }

        for y in 0..height {
            let bottom_idx = if y + r <= max_y {
                (y + r) * stride + col
            } else {
                last
            };
            let top_idx = if y > r {
                (y - r - 1) * stride + col
            } else {
                first
            };
            let out_idx = y * stride + col;

            for ch in 0..4 {
                acc[ch] += src[bottom_idx + ch] as i32 - src[top_idx + ch] as i32;
                dst[out_idx + ch] = ((acc[ch] as f32 * iarr).round()).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_preserves_solid_color_and_diffuses_impulse() {
        let mut solid = vec![120u8; 8 * 8 * 4];
        blur_rgba(&mut solid, 8, 8, 4.0);
        assert!(solid.iter().all(|&b| (b as i32 - 120).abs() <= 1));

        let mut impulse = vec![0u8; 9 * 9 * 4];
        let center = (4 * 9 + 4) * 4;
        impulse[center] = 255;
        blur_rgba(&mut impulse, 9, 9, 2.5);
        assert!(impulse[center] < 255);
        assert!(impulse[center + 4] > 0);
    }
}
