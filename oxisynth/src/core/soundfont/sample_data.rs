use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::Arc,
};

use soundfont::raw::SampleChunk;

#[derive(Debug, Clone)]
pub struct SampleData(Arc<[i16]>);

impl SampleData {
    #[cfg_attr(not(feature = "sf3"), allow(dead_code))]
    pub fn new(data: Arc<[i16]>) -> Self {
        Self(data)
    }
    
    pub fn load_sample_data<F: Read + Seek>(
        file: &mut F,
        smpl: &SampleChunk,
        header: &soundfont::raw::SampleHeader,
    ) -> io::Result<Arc<[i16]>> {
        let data = Self::load_full(file, smpl, header.start * 2, (header.end - header.start + 1) * 2)?;
        Ok(data.0)
    }

    pub fn load<F: Read + Seek>(
        file: &mut F,
        smpl: &SampleChunk,
    ) -> io::Result<Self> {
        Self::load_full(file, smpl, 0, smpl.len)
    }

    fn load_full<F: Read + Seek>(file: &mut F, smpl: &SampleChunk, offset_bytes: u32, length_bytes: u32) -> io::Result<Self> {
        let sample_pos = smpl.offset + (offset_bytes as u64);

        if let Err(err) = file.seek(SeekFrom::Start(sample_pos)) {
            log::error!("Failed to seek position in data file: {err}");
            return Err(err);
        }

        let mut data = vec![0i16; length_bytes as usize / 2];

        {
            let byte_slice = crate::unsafe_stuff::slice_i16_to_u8_mut(&mut data);

            if let Err(err) = file.read_exact(byte_slice) {
                log::error!("Failed to read sample data: {err}");
                return Err(err);
            }
        }

        // Sample is in LittleEndian so if we are on BigEndian flip the bits around?
        // TODO: Not sure if this is working as expected, gotta test this in a VM
        if cfg!(target_endian = "big") {
            for n in data.iter_mut() {
                *n = n.to_le();
            }
        }

        Ok(Self(data.into()))
    }

    #[cfg_attr(not(feature = "sf3"), allow(dead_code))]
    pub fn as_byte_slice(&self) -> &[u8] {
        crate::unsafe_stuff::slice_i16_to_u8(&self.0)
    }
}

impl std::ops::Deref for SampleData {
    type Target = [i16];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
