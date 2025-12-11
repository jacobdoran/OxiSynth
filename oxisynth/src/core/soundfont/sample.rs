use std::sync::Arc;

use soundfont::raw::SampleLink;

use crate::error::LoadError;

use crate::unsafe_stuff;

use super::SampleData;

#[derive(Clone, Debug)]
pub struct Sample {
    name: Arc<str>,

    start: u32,
    end: u32,

    loop_start: u32,
    loop_end: u32,

    origpitch: u8,
    pitchadj: i8,

    sample_rate: u32,
    sample_type: SampleLink,
    data: SampleData,

    /// The amplitude, that will lower the level of the sample's loop to
    /// the noise floor. Needed for note turnoff optimization, will be
    /// filled out automatically
    amplitude_that_reaches_noise_floor: Option<f64>,
}

impl Sample {
    pub fn import_raw(
        sample_header: &soundfont::raw::SampleHeader,
        raw_bytes: &[u8],        
    ) -> Result<Sample, LoadError> {

        // Create a modified header with adjusted offsets for the sliced data
        let mut sample = Sample {
            name: sample_header.name.clone().into(),
            start: sample_header.start,
            end: sample_header.end,
            loop_start: sample_header.loop_start,
            loop_end: sample_header.loop_end,
            sample_rate: sample_header.sample_rate,
            origpitch: sample_header.origpitch,
            pitchadj: sample_header.pitchadj,
            sample_type: sample_header.sample_type,
            data: SampleData::new([].into()),
            amplitude_that_reaches_noise_floor: None,
        };

        #[cfg(feature = "sf3")]
        if sample.sample_type.is_vorbis() {
            use lewton::inside_ogg::OggStreamReader;
            use std::io::Cursor;
            let buf = Cursor::new(raw_bytes);
            let mut reader = match OggStreamReader::new(buf) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to create OGG reader for sample {}: {:?}", sample.name, e);
                    return Err(LoadError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Invalid OGG data: {:?}", e)
                    )));
                }
            };

            // Pre-allocate with estimated capacity based on sample rate and duration
            // Average soundfont sample is ~2 seconds, allocate for 4 to avoid most reallocations
            let estimated_samples = (sample.sample_rate * 4) as usize;
            let mut new = Vec::with_capacity(estimated_samples);
            
            loop {
                match reader.read_dec_packet_itl() {
                    Ok(Some(pck)) => {
                        // Use extend_from_slice instead of append - faster for slices
                        new.extend_from_slice(&pck);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        log::error!("Error reading Vorbis packet for sample {}: {:?}", sample.name, e);
                        return Err(LoadError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Vorbis decode error: {:?}", e)
                        )));
                    }
                }
            }

            sample.start = 0;
            sample.end = (new.len() - 1) as u32;
            sample.data = SampleData::new(new.into());

            // loop is fowled?? (cluck cluck :)
            if sample.loop_end > sample.end
                || sample.loop_start >= sample.loop_end
                || sample.loop_start <= sample.start
            {
                // can pad loop by 8 samples and ensure at least 4 for loop (2*8+4)
                if (sample.end - sample.start) >= 20 {
                    sample.loop_start = sample.start + 8;
                    sample.loop_end = sample.end - 8;
                } else {
                    // loop is fowled, sample is tiny (can't pad 8 samples)
                    sample.loop_start = sample.start + 1;
                    sample.loop_end = sample.end - 1;
                }
            }

            // Mark it as no longer compressed sample
            sample.sample_type = match sample.sample_type {
                SampleLink::VorbisMonoSample => SampleLink::MonoSample,
                SampleLink::VorbisRightSample => SampleLink::RightSample,
                SampleLink::VorbisLeftSample => SampleLink::LeftSample,
                SampleLink::VorbisLinkedSample => SampleLink::LinkedSample,
                _ => unreachable!("Not Vorbis"),
            };
        } else {
            // Convert raw bytes to i16 samples
            let i16_data = unsafe_stuff::slice_u8_to_i16(raw_bytes);
            sample.data = SampleData::new(i16_data.into());

            // take away start offset for sample data as we already sliced it
            let offset = sample.start;
            sample.loop_start -= offset;
            sample.loop_end -= offset;
            sample.start-= offset;
            sample.end -= offset;
        }
        if sample.end - sample.start < 8 {
            log::warn!(
                "Ignoring sample {:?}: too few sample data points",
                sample.name
            );
        } else if sample.sample_type.is_rom() {
            log::warn!("Ignoring sample: can't use ROM samples");
        } else {
            // Optimize only valid samples
            sample.optimize_sample();
        }

        Ok(sample)
    }

    pub fn import(
        sample_header: &soundfont::raw::SampleHeader,
        data: SampleData,
    ) -> Result<Sample, LoadError> {
        // end is inclusive (the last valid sample index), so add 1 for exclusive slice end
        let sample_data = &data[sample_header.start as usize..=sample_header.end as usize];
        // convertt to u8 slice
        let sample_data = sample_data
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect::<Vec<u8>>();
        let sample = Sample::import_raw(sample_header, &sample_data)?;
        Ok(sample)
    }

    /// - Scan the loop
    /// - determine the peak level
    /// - Calculate, what factor will make the loop inaudible
    /// - Store in sample
    fn optimize_sample(&mut self) {
        if self.sample_type.is_vorbis() {
            return;
        }

        if self.amplitude_that_reaches_noise_floor.is_none() {
            let mut peak_max = 0;
            let mut peak_min = 0;

            // Scan the loop
            for i in self.loop_start..self.loop_end {
                let val = self.data[i as usize] as i32;
                if val > peak_max {
                    peak_max = val
                } else if val < peak_min {
                    peak_min = val
                }
            }

            // Determine the peak level
            let peak = if peak_max > -peak_min {
                peak_max
            } else {
                -peak_min
            };

            // Avoid division by zero
            let peak = if peak == 0 { 1 } else { peak };

            // Calculate what factor will make the loop inaudible
            // For example: Take a peak of 3277 (10 % of 32768).  The
            // normalized amplitude is 0.1 (10 % of 32768).  An amplitude
            // factor of 0.0001 (as opposed to the default 0.00001) will
            // drop this sample to the noise floor.

            // 16 bits => 96+4=100 dB dynamic range => 0.00001
            let normalized_amplitude_during_loop = peak as f32 / 32768.0;
            let result = 0.00003 / normalized_amplitude_during_loop as f64;

            // Store in sample
            self.amplitude_that_reaches_noise_floor = Some(result);
        }
    }

    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline(always)]
    pub fn start(&self) -> u32 {
        self.start
    }

    #[inline(always)]
    pub fn end(&self) -> u32 {
        self.end
    }

    #[inline(always)]
    pub fn amplitude_that_reaches_noise_floor(&self) -> Option<f64> {
        self.amplitude_that_reaches_noise_floor
    }

    #[inline(always)]
    pub fn loop_start(&self) -> u32 {
        self.loop_start
    }

    #[inline(always)]
    pub fn loop_end(&self) -> u32 {
        self.loop_end
    }

    #[inline(always)]
    pub fn origpitch(&self) -> u8 {
        self.origpitch
    }

    #[inline(always)]
    pub fn pitchadj(&self) -> i8 {
        self.pitchadj
    }

    #[inline(always)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[inline(always)]
    pub fn sample_type(&self) -> SampleLink {
        self.sample_type
    }

    #[inline(always)]
    pub fn data(&self) -> &[i16] {
        &self.data
    }
}
