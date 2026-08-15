use rtrb::{Consumer, Producer, RingBuffer};

pub struct AudioBridge {
    pub producer: Producer<f32>,
    pub consumer: Consumer<f32>,
}

impl AudioBridge {
    /// Instantiates a new lock-free ring buffer on the heap with a fixed sample capacity.
    /// This should be initialized once inside Thread A during the application startup phase.

    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "Ring buffer capacity must be greater than zero!"
        );
        let (producer, consumer) = RingBuffer::<f32>::new(capacity);
        Self { producer, consumer }
    }

    /// Splits the bridge into its distinct standalone owner halves.
    /// Ownership of the Producer is moved to Thread B (Audio I/O).
    /// Ownership of the Consumer is moved to Thread C (DSP Worker).

    pub fn split(self) -> (Producer<f32>, Consumer<f32>) {
        (self.producer, self.consumer)
    }
}

#[cfg(test)]
/// **Tests were created using AI. no part of the original project implementation used AI.**
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_lock_free_spsc_bridge_stress_and_order() {
        // 1. Setup an 8192-sample bridge cushion (matching topology spec)
        let bridge = AudioBridge::new(8192);
        let (mut producer, mut consumer) = bridge.split();

        let total_test_samples = 50_000;

        // 2. Spawn Mock Thread B: The Producer (simulating rapid cpal microphone inputs)
        let tx_handle = thread::spawn(move || {
            let mut current_value = 1.0f32;
            let mut samples_sent = 0;

            while samples_sent < total_test_samples {
                // Attempt to push sequential float values into the ring buffer slot
                if producer.push(current_value).is_ok() {
                    current_value += 1.0;
                    samples_sent += 1;
                } else {
                    // Buffer is temporarily full; yield execution to mimic live hardware intervals
                    thread::yield_now();
                }
            }
        });

        // 3. Spawn Mock Thread C: The Consumer (simulating continuous DSP polling loop iterations)
        let rx_handle = thread::spawn(move || {
            let mut expected_value = 1.0f32;
            let mut samples_received = 0;

            while samples_received < total_test_samples {
                // Poll the tail of the lock-free ring buffer for new data samples
                if let Ok(value) = consumer.pop() {
                    // Assert that values arrive completely uncorrupted and in strict chronological order
                    assert_eq!(
                        value, expected_value,
                        "Data reordering or corruption detected inside the lock-free bridge stream!"
                    );
                    expected_value += 1.0;
                    samples_received += 1;
                } else {
                    // Buffer is temporarily empty; sleep momentarily to simulate high-priority math cycles
                    thread::sleep(Duration::from_micros(50));
                }
            }
        });

        // Wait for both execution environments to finish their tasks cleanly
        tx_handle.join().unwrap();
        rx_handle.join().unwrap();
    }
}
