#[must_use]
pub fn empty_visibility_blob(width: u16, height: u16) -> Vec<u8> {
    let cell_count = usize::from(width) * usize::from(height);
    vec![0; cell_count.div_ceil(8)]
}

pub fn set_visibility_bit(blob: &mut [u8], cell_index: usize) {
    let byte_index = cell_index / 8;
    let bit_index = cell_index % 8;
    if let Some(byte) = blob.get_mut(byte_index) {
        *byte |= 1 << bit_index;
    }
}

#[must_use]
pub fn read_visibility_bit(blob: &[u8], cell_index: usize) -> bool {
    let byte_index = cell_index / 8;
    let bit_index = cell_index % 8;
    blob.get(byte_index)
        .map(|byte| byte & (1 << bit_index) != 0)
        .unwrap_or(false)
}
