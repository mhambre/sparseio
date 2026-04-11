use std::collections::HashSet;

/// Visualizes how extents of a sparse object are being filled by rendering a horizontal bar. Each character
/// in the bar represents a portion of the object, with different symbols indicating the fill status.
pub fn render_sparse_fill_bar(filled_offsets: &HashSet<usize>, chunk_offsets: &[usize], width: usize) -> String {
    if chunk_offsets.is_empty() {
        return String::new();
    }

    let width = width.max(1).min(chunk_offsets.len());
    let total = chunk_offsets.len();
    let mut bar = String::with_capacity(width);

    for bucket in 0..width {
        let start = (bucket * total) / width;
        let end = ((bucket + 1) * total) / width;

        let mut bucket_total = 0usize;
        let mut bucket_filled = 0usize;
        for &offset in &chunk_offsets[start..end] {
            bucket_total += 1;
            if filled_offsets.contains(&offset) {
                bucket_filled += 1;
            }
        }

        let symbol = if bucket_filled == 0 {
            '░'
        } else if bucket_filled == bucket_total {
            '█'
        } else {
            '▓'
        };
        bar.push(symbol);
    }

    bar
}
