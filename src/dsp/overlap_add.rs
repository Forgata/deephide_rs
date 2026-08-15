use crate::payload_engine::payload::frame;

pub struct OverlapAddBuffer {
    frame_size: usize,
    hop_size: usize,
    accumulator: Vec<f32>,
}

impl OverlapAddBuffer {
    /// Initializes a cleared overlap-add accumulation buffer.
    pub fn new(frame_size: usize, hop_size: usize) -> Self {
        assert_eq!(
            frame_size, 1024,
            "DeepHide is strictly optimized for 1024-sample frames"
        );
        assert_eq!(
            hop_size, 512,
            "DeepHide is strictly optimized for a 512-sample hop size"
        );

        Self {
            frame_size,
            hop_size,
            accumulator: vec![0.0f32; frame_size],
        }
    }

    /// Blends a newly synthesized 1024-sample IFFT frame into the timeline,
    /// extracts the finished 512-hop block, and shifts the remaining history left.
    ///
    /// ### Arguments
    /// * `ifft_frame` - The raw 1024-sample real-valued slice output from the Inverse FFT.
    /// * `hop_out` - A pre-allocated 512-sample destination slice for the completed audio.
    ///
    /// ### Thread Safety Notice
    /// This function performs ZERO heap allocations and mutates existing slices in-place.

    pub fn process_hop(&mut self, ifft_frame: &[f32], hop_out: &mut [f32]) {
        assert_eq!(
            ifft_frame.len(),
            self.frame_size,
            "Input IFFT frame size mismatch!"
        );
        assert_eq!(
            hop_out.len(),
            self.hop_size,
            "Output hop buffer size mismatch!"
        );

        for i in 0..self.frame_size {
            self.accumulator[i] += ifft_frame[i];
        }

        hop_out.copy_from_slice(&self.accumulator[0..self.hop_size]);
        self.accumulator
            .copy_within(self.hop_size..self.frame_size, 0);

        self.accumulator[self.hop_size..self.frame_size].fill(0.0);
    }

    /// Resets the internal accumulator back to silence.
    /// Essential when switching tracks or starting a fresh bitstream stream.
    pub fn clear(&mut self) {
        self.accumulator.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlap_add_mixing_and_sliding_in_place() {
        let mut ola = OverlapAddBuffer::new(1024, 512);

        // Create two mock sequential IFFT blocks consisting of pure 1.0s
        let frame_a = vec![1.0f32; 1024];
        let frame_b = vec![1.0f32; 1024];

        let mut output_hop_1 = vec![0.0f32; 512];
        let mut output_hop_2 = vec![0.0f32; 512];

        // Process first hop: accumulator becomes [1.0; 1024].
        // Front 512 are sliced out (should be 1.0). Back 512 shift left.
        ola.process_hop(&frame_a, &mut output_hop_1);
        for &sample in &output_hop_1 {
            assert_eq!(sample, 1.0);
        }

        // Process second hop: shifted values (1.0) add with new frame values (1.0).
        // Front 512 are sliced out (should be 1.0 + 1.0 = 2.0 due to perfect overlap).
        ola.process_hop(&frame_b, &mut output_hop_2);
        for &sample in &output_hop_2 {
            assert_eq!(sample, 2.0);
        }
    }
}
