# DeepHide - Audio Steganography System (Rust Implementation)

## Overview

DeepHide is a **real-time audio steganography system** implemented in **Rust** that hides and encrypts files within audio signals using subtle phase modulation of the FFT spectrum. The system embeds data into audio by modulating the phase of safe frequency bins using **Direct Sequence Spread Spectrum (DSSS)** technology with psychoacoustic masking analysis.

## Key Features

- **Steganographic Audio Modulation**: Embeds data by subtly shifting phase of FFT spectrum bins identified as psychoacoustically "safe"
- **End-to-End Encryption**: AES-256-GCM encryption with PBKDF2 key derivation (600,000 iterations)
- **Forward Error Correction**: Reed-Solomon erasure coding (3+3 shards) with block interleaving for burst error resilience
- **DSSS Spreading**: 64-chip spreading factor using deterministic LFSR-based PN sequences
- **Real-time Multi-threaded Architecture**: Lock-free SPSC ring buffers separate audio I/O from DSP processing
- **Zero-Allocation DSP Loop**: Pre-allocated buffers and in-place FFT for deterministic real-time performance

## System Architecture

### Thread Topology (3-Thread Design)

| Thread                      | Priority | Responsibility                                                                      |
| --------------------------- | -------- | ----------------------------------------------------------------------------------- |
| **Thread A** (Main)         | Low      | File I/O, payload preparation (encryption, FEC, interleaving), orchestration        |
| **Thread B** (Hardware I/O) | Maximum  | `cpal` audio capture → lock-free ring buffer (Producer)                             |
| **Thread C** (DSP Worker)   | High     | Ring buffer (Consumer) → STFT → DSSS embedding → IFFT → overlap-add → output buffer |

### Data Flow

1. **Preparation (Thread A)**:
   - Load file from `data/` directory
   - Frame with protocol header (filename, size)
   - Encrypt using AES-256-GCM with key derived from password + random salt (PBKDF2, 600k iterations)
   - Packetize into 256-byte frames
   - Apply Reed-Solomon FEC (3 data + 3 parity shards per block)
   - Interleave shards across blocks for burst error distribution
   - Serialize to bitstream and inject 80-bit high-entropy preamble

2. **Modulation (Thread C - Real-time)**:
   - Accumulate 1024 samples from ring buffer (512-sample hop)
   - Apply Hamming window
   - Forward STFT (in-place, pre-planned FFT)
   - Psychoacoustic analysis: compute Bark-scale masking thresholds
   - Identify "safe bins" where signal energy exceeds masking threshold
   - Spread next payload bit using 64-chip PN sequence (DSSS)
   - Phase-modulate safe bins: `new_phase = original_phase ± delta` (delta = 0.02 rad)
   - Inverse STFT → overlap-add reconstruction (512-sample hop)
   - Push modulated samples to output ring buffer

3. **Output (Thread D - Background)**:
   - Consume modulated samples from output ring buffer
   - Write to WAV file (`recorded_secret.wav`)

### Core Modules

```
src/
├── main.rs                 # Thread orchestration, entry point
├── pipeline.rs             # Offline file-to-file processing (embed + extract)
├── payload_engine.rs       # Encryption, FEC, framing, serialization, preamble
├── threads/
│   ├── bridge.rs           # Lock-free SPSC ring buffer (rtrb)
│   ├── hardware.rs         # cpal audio input thread (Thread B)
│   └── worker.rs           # Real-time DSP worker (Thread C)
├── dsp/
│   ├── fft.rs              # Pre-planned FFT workspace (rustfft)
│   ├── window.rs           # Hamming window
│   ├── overlap_add.rs      # Overlap-add buffer for reconstruction
│   └── psychoacoustic.rs   # Bark-scale masking threshold computation
├── modulation/
│   ├── embed.rs            # Phase modulation in frequency domain
│   ├── spreader.rs         # DSSS spreading/despreading (64-chip)
│   └── pn_gen.rs           # LFSR-based deterministic PN generator
└── payload_engine/
    ├── crypto/
    │   ├── derive_key.rs   # PBKDF2-HMAC-SHA256 key derivation
    │   └── aes_gcm_encryption.rs  # AES-256-GCM encrypt/decrypt
    ├── fec/
    │   ├── forward_err_correction.rs  # Reed-Solomon encode/decode
    │   └── fec_interleaver.rs         # Block interleaving/deinterleaving
    ├── payload/
    │   ├── frame.rs        # Protocol framing/deframing
    │   └── packetize.rs    # Packetization with sequence IDs
    ├── preamble.rs         # 80-bit preamble inject/strip
    └── serializer.rs       # Bit serialization/deserialization
```

## Getting Started

### Prerequisites

- Rust 1.75+ (2024 edition)
- Audio input device (for real-time mode)

### Build

```bash
cargo build --release
```

### Run (Real-time Embedding)

```bash
cargo run --release
```

This will:

1. Read `data/example.txt`
2. Encrypt with password `"1234"`
3. Capture audio from default microphone
4. Embed data in real-time
5. Write modulated audio to `recorded_secret.wav`

### Offline Processing (File-to-File)

The `OfflinePipeline` in `pipeline.rs` provides a non-real-time API for batch processing:

```rust
use deephide_rs::pipeline::OfflinePipeline;

let mut pipeline = OfflinePipeline::new(48000);
pipeline.process_file("input.wav", "output.wav", &payload_bits, &pn_sequence)?;
```

### Recovery

Recovery implementation extracts bits from modulated audio by:

1. Computing reference phases from clean audio (or estimating)
2. Measuring phase deltas in safe bins
3. Correlating against known PN sequences (despreading)
4. FEC decoding → decryption → deframing

## Configuration

Key parameters (defined in code):

| Parameter         | Value             | Location                              |
| ----------------- | ----------------- | ------------------------------------- |
| Sample Rate       | 48 kHz            | `main.rs`, `pipeline.rs`              |
| Frame Size        | 1024 samples      | `dsp/fft.rs`, `threads/worker.rs`     |
| Hop Size          | 512 samples       | `dsp/overlap_add.rs`                  |
| Spreading Factor  | 64 chips/bit      | `modulation/spreader.rs`              |
| Phase Delta       | 0.02 radians      | `modulation/embed.rs`                 |
| FEC Configuration | 3 data + 3 parity | `payload_engine/fec/`                 |
| Packet Size       | 256 bytes         | `payload_engine/payload/packetize.rs` |
| PBKDF2 Iterations | 600,000           | `payload_engine/crypto/derive_key.rs` |

## Dependencies

```toml
[dependencies]
pbkdf2 = "0.13"           # Key derivation
sha2 = "0.11"             # SHA-256 for PBKDF2
hmac = "0.13"             # HMAC for PBKDF2
rand = "0.10"             # Random salt generation
rand_chacha = "0.10"      # ChaCha RNG
aes-gcm = "0.11"          # AES-256-GCM
reed-solomon-erasure = "6.0"  # FEC
rustfft = "6.4"           # FFT processing
hound = "3.5"             # WAV file I/O
rtrb = "0.3"              # Lock-free SPSC ring buffer
cpal = "0.18"             # Cross-platform audio I/O
```

## Testing

```bash
cargo test
```

Tests cover:

- Payload engine round-trip (encryption → FEC → interleaving → serialization → preamble → reverse)
- FEC failure detection (exceeding 3 erasures per block)
- DSSS spreading/despreading correctness
- Phase embedding with conjugate mirroring
- Real-time DSP worker thread execution

## Architecture References

- `architecture.md` - High-level system architecture
- `thread_architecture.md` - Detailed concurrency model and thread topology

## License

MIT

1. Prepare your file (e.g., `file.txt`)
2. Run the application with appropriate parameters
3. The system will generate `output.wav` with the embedded data

## Technical Notes

### Key Parameters

- Frame size: 1024 samples
- Hop size: 512 samples
- DSSS spreading factor: 64 chips
- FEC configuration: 3 data shards, 3 parity shards
- AES-256-GCM encryption with 12-byte nonce

### Security Considerations

- Key derivation uses PBKDF2 with 600,000 iterations
- Salt is randomly generated for each payload
- Authentication tag included in encryption
- Preamble provides synchronization capability

## License

MIT License

© Forgata - 2026
