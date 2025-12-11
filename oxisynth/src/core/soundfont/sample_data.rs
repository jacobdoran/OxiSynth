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
    ) -> io::Result<Arc<[u8]>> {
        // For Vorbis samples in SF3, start/end are already in terms of i16 positions (bytes/2)
        // For regular samples, start/end are sample indices, so we need to multiply by 2 for bytes
        let (offset_bytes, length_bytes) = if header.sample_type.is_vorbis() {
            // Vorbis: start and end are i16 indices into compressed data
            // Number of i16 values = (end - start + 1), bytes = that * 2
            (header.start, (header.end - header.start))
        } else {
            // Regular: start and end are sample indices
            // Each sample is 2 bytes (i16), so multiply by 2
            (header.start * 2, (header.end - header.start + 1) * 2)
        };
        let data = Self::load_full(file, smpl, offset_bytes, length_bytes)?;
        Ok(data)
    }

    pub fn load<F: Read + Seek>(
        file: &mut F,
        smpl: &SampleChunk,
    ) -> io::Result<Self> {
        let data = Self::load_full(file, smpl, 0, smpl.len)?;
        // convert data from u8 to i16
        let i16_data = bytemuck::cast_slice(&data);
        Ok(Self(i16_data.into()))
    }

    fn load_full<F: Read + Seek>(file: &mut F, smpl: &SampleChunk, offset_bytes: u32, length_bytes: u32) -> io::Result<Arc<[u8]>> {
        let sample_pos = smpl.offset + (offset_bytes as u64);

        if let Err(err) = file.seek(SeekFrom::Start(sample_pos)) {
            log::error!("Failed to seek position in data file: {err}");
            return Err(err);
        }

        let mut data = vec![0u8; length_bytes as usize];

        if let Err(err) = file.read_exact(&mut data) {
            log::error!("Failed to read sample data: {err}");
            return Err(err);
        }

        // Sample is in LittleEndian so if we are on BigEndian flip the bits around?
        // TODO: Not sure if this is working as expected, gotta test this in a VM
        if cfg!(target_endian = "big") {
            for chunk in data.chunks_exact_mut(2) {
                chunk.swap(0, 1);
            }
        }

        Ok(data.into())
    }

    #[cfg_attr(not(feature = "sf3"), allow(dead_code))]
    pub fn as_byte_slice(&self) -> &[i16] {
        &self.0
    }
}

impl std::ops::Deref for SampleData {
    type Target = [i16];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
