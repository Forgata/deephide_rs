# DeepHide - Audio-Based Data Hiding System

## Overview

DeepHide is a real-time Node.js/TypeScript system that hides and encrypts files within audio signals using subtle human speech modulation. The system embeds data into audio by modulating the FFT spectrum of audio frames using Direct Sequence Spread Spectrum (DSSS) technology.

## Key Features

- **Steganographic Audio Modulation**: Embeds data into audio by subtly modifying FFT spectrum
- **End-to-End Encryption**: AES-256-GCM encryption with password-derived keys
- **Forward Error Correction**: Reed-Solomon erasure coding for robust data recovery
- **DSSS Spreading**: Uses pseudo-random sequences to spread bits across frequency bins
- **Real-time Processing**: Captures audio in real-time and modulates data into the audio signal

## System Architecture

### Data Flow

1. **Preparation**:
   - File is framed with header containing filename and size
   - Payload is encrypted using AES-256-GCM with key derived from password and random salt
   - FEC (Reed-Solomon) adds redundancy for error correction
   - Data is interleaved to distribute errors across blocks
   - Bitstream is serialized and a high-entropy preamble is injected

2. **Modulation**:
   - Bitstream is converted to symbols and spread using DSSS
   - PN sequences generate spreading chips (+1/-1 values)
   - Chips are embedded into safe frequency bins of FFT spectrum
   - Audio is processed through STFT, IFFT, and overlap-add

3. **Transmission**:
   - Modulated audio is captured via microphone using PvRecorder
   - Audio is processed in frames (1024 samples) with Hamming window
   - Each frame undergoes STFT processing to extract and embed data

4. **Recovery**:
   - Receiver captures audio and processes in frames
   - Hunt mode searches for preamble pattern to acquire synchronization
   - Once synced, extracts bits from safe frequency bins
   - FEC decoding recovers original data packets
   - Decryption reveals the original file

### Core Components

- **Embedding Module** (`src/core/embedding`):
  - `generator.ts`: Main payload preparation function
  - `framing.ts`: Adds protocol headers to data packets
  - `packer.ts`: Splits encrypted data into frames with identifiers
  - `crypto/`: Key derivation and encryption modules
  - `fec/`: Forward Error Correction using Reed-Solomon
  - `bitstream/`: Serialization and preamble injection

- **Modulation Layer** (`src/core/modulator`):
  - `dsss.ts`: DSSS spreading logic
  - `pnGen.ts`: Pseudo-random sequence generator (LFSR-based)
  - `spreader.ts`: Maps bits to spreading chips
  - `embedChips.ts`: Embeds chips into FFT spectrum

- **Audio Processing** (`src/core/profiler`):
  - `recorder.ts`: Captures audio and processes data frames
  - `processFrame.ts`: Handles STFT, chip embedding, and IFFT
  - `fft.ts`, `freqBarkMap.ts`, `masking.ts`: Audio analysis utilities

- **Receiver Module** (`src/core/receiver`):
  - `receiver.ts`: Main receiver logic with hunt/synced modes
  - `despreader.ts`: Recovers bits from frequency bins
  - `binExtractor.ts`, `bitReconstructor.ts`, `decodeFEC.ts`, `decryptor.ts`: Data recovery components

## Getting Started

### Installation

```bash
npm install
```

### Build

```bash
npm run build
```

### Run

```bash
npm run dev
```

### Recovery

```bash
npm run recover
```

## Usage

### Embedding Data

The system automatically processes files when running in development mode. To embed data manually:

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

## Testing

The project includes comprehensive tests for all modules. Run tests with:

```bash
npm test
```

## License

MIT License

© Forgata - 2026
