//! All the unsafe code gathered in a single place, to keep it enclosed

/// [i16] -> [u8] conversion
pub fn slice_i16_to_u8(slice: &[i16]) -> &[u8] {
    let len = std::mem::size_of_val(slice);
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, len) }
}

/// [i16] -> [u8] conversion
pub fn slice_i16_to_u8_mut(slice: &mut [i16]) -> &mut [u8] {
    let len = std::mem::size_of_val(slice);
    unsafe { std::slice::from_raw_parts_mut(slice.as_ptr() as *mut u8, len) }
}

/// [f32] -> [u8] conversion
#[cfg_attr(not(test), allow(dead_code))]
pub fn slice_f32_to_u8(slice: &[f32]) -> &[u8] {
    let len = std::mem::size_of_val(slice);
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, len) }
}

/// [u8] -> [i16] conversion
#[cfg_attr(not(test), allow(dead_code))]
pub fn slice_u8_to_i16(slice: &[u8]) -> &[i16] {
    let len = std::mem::size_of_val(slice) / std::mem::size_of::<i16>();
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const i16, len) }
}
