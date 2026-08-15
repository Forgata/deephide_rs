use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::Producer;
/*
/// Handles initializing and managing the life cycle of the hardware microphone stream.
/// Operates completely allocation-free inside its high-priority audio callback loop.
pub struct HardwareInputDevice {
    stream: cpal::Stream,
}

impl HardwareInputDevice {
    /// Discovers the system's default input device and instantiates a high-priority
    /// CPAL recording stream, feeding captured samples straight into the lock-free Producer.
    ///
    /// # Arguments
    /// * `target_sample_rate` - Typically 44100 Hz to align with your DSP math layouts.
    /// * `mut producer` - The active writing half of your lock-free SPSC AudioBridge.
    pub fn new(target_sample_rate: u32, mut producer: Producer<f32>) -> Result<Self, String> {
        // 1. Initialize the default operating system audio host (CoreAudio, ALSA, or WASAPI)
        let host = cpal::default_host();

        // 2. Locate the default physical input device (the system microphone)
        let device = host.default_input_device().ok_or_else(|| {
            "Hardware: No default audio input device found on this system!".to_string()
        })?;

        // 3. Establish specific mono stream configurations matching our DSP expectations
        let config = cpal::StreamConfig {
            channels: 1, // Force mono recording to maintain a flat time-domain channel
            sample_rate: cpal::SampleRate::from(target_sample_rate),
            buffer_size: cpal::BufferSize::Default, // Let the OS determine optimal latency sizes
        };

        // 4. Construct the high-priority input stream callback closure
        // Inside this data block, we are strictly bound by real-time safety rules!
        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // HIGH PRIORITY LOOP: No Vecs, no Strings, no Mutexes, no println!
                    for &sample in data {
                        // Push raw float samples directly into our atomic shock-absorber queue
                        if producer.push(sample).is_err() {
                            // Buffer is full. In a live system, we drop the frame to prevent blocks.
                            // We do NOT print an error here because console logging can block!
                        }
                    }
                },
                move |_err| {
                    // Handle hardware device disconnects or stream drops gracefully
                    // We avoid inline string allocations inside this real-time boundary block
                },
                None, // No timeout thresholds needed
            )
            .map_err(|e| {
                format!(
                    "Hardware: Failed to construct CPAL input stream structure: {}",
                    e
                )
            })?;

        Ok(Self { stream })
    }

    /// Signals the underlying host system to start drawing active audio frames from the hardware.
    pub fn play(&self) -> Result<(), String> {
        self.stream
            .play()
            .map_err(|e| format!("Hardware: Failed to initiate stream playback engine: {}", e))
    }

    /// Signals the underlying host system to temporarily pause the physical input listeners.
    pub fn pause(&self) -> Result<(), String> {
        self.stream
            .pause()
            .map_err(|e| format!("Hardware: Failed to suspend stream playback engine: {}", e))
    }
}

*/

pub struct HardwareInputDevice {
    stream: cpal::Stream,
}

impl HardwareInputDevice {
    pub fn new(_target_sample_rate: u32, mut producer: Producer<f32>) -> Result<Self, String> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or_else(|| "Hardware: No default audio input device found!".to_string())?;

        // 1. Query the OS for the microphone's actual native running format
        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Hardware: Failed to query default stream config: {}", e))?;

        // Extract native parameters to adjust our channel reading matrix
        let num_channels = default_config.channels() as usize;
        let stream_config = default_config.config();

        println!(
            "Hardware: Locking device using native format: Channels: {}, Sample Rate: {}Hz",
            num_channels, stream_config.sample_rate
        );

        // 2. Build the hardware stream callback using the native config the OS demands
        let stream = device
            .build_input_stream(
                stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Process samples in exact channel frames (stride = num_channels)
                    // This strips multi-channel inputs back into the required mono stream
                    for frame in data.chunks_exact(num_channels) {
                        // Extract channel 0 to maintain absolute allocation-free mono compatibility
                        let mono_sample = frame[0];

                        if producer.push(mono_sample).is_err() {
                            // Buffer full cushion boundary hit. Drop gracefully.
                        }
                    }
                },
                move |_err| {},
                None,
            )
            .map_err(|e| {
                format!(
                    "Hardware: Failed to construct CPAL input stream structure: {}",
                    e
                )
            })?;

        Ok(Self { stream })
    }

    pub fn play(&self) -> Result<(), String> {
        self.stream
            .play()
            .map_err(|e| format!("Hardware: Failed to engage stream playback engine: {}", e))
    }

    pub fn pause(&self) -> Result<(), String> {
        self.stream
            .pause()
            .map_err(|e| format!("Hardware: Failed to suspend stream playback engine: {}", e))
    }
}
