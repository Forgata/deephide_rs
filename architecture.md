# DeepHide System Architecture

## System Overview

DeepHide is a real-time audio steganography system that hides and encrypts files within audio signals using subtle human speech modulation. The system operates as a complete pipeline from input file to audio output, with corresponding recovery capabilities.

## Architecture Components

### 1. Data Preparation Module

**Location**: `src/core/embedding/`

- **File**: `generator.ts`
- **Function**: `preparePayload()`
- **Process**:
  1. Loads input file as raw bytes
  2. Frames data with protocol header (filename, size)
  3. Encrypts payload using AES-256-GCM with password-derived key
  4. Applies Forward Error Correction (Reed-Solomon) for redundancy
  5. Interleaves data to distribute errors across blocks
  6. Serializes to bitstream and injects high-entropy preamble

### 2. Modulation Layer

**Location**: `src/core/modulator/`

- **Key Components**:
  - **DSSS Spreader**: `spreader.ts` - Maps bits to spreading chips using PN sequences
  - **PN Generator**: `pnGen.ts` - Generates pseudo-random sequences via LFSR
  - **Embedding**: `embedChips.ts` - Embeds chips into FFT spectrum of audio frames
  - **Framing**: `framing.ts` - Adds protocol headers for data identification

### 3. Audio Processing Pipeline

**Location**: `src/core/profiler/`

- **Recorder**: `recorder.ts` - Captures audio and processes data frames
- **Processing**: `processFrame.ts` - Handles STFT, chip embedding, and IFFT
- **Audio Analysis**: `fft.ts`, `freqBarkMap.ts`, `masking.ts` - Identify safe frequency bins for data embedding

### 4. Transmission

- Uses microphone input via `PvRecorder` (Picovoice)
- Processes audio in 1024-sample frames with Hamming window
- Modulates data into frequency bins during STFT processing

### 5. Recovery System

**Location**: `src/core/receiver/`

- **Hunt Mode**: Searches for preamble pattern to acquire synchronization
- **Synced Mode**: Extracts bits from safe frequency bins once synchronized
- **FEC Decoder**: `decodeFEC.js` - Recovers original data packets
- **Decryptor**: `decryptor.js` - Decrypts payload using derived key
- **Bit Reconstruction**: `bitReconstructor.js` - Rebuilds original bitstream

## Data Flow

1. **Input**: File to be hidden (`file.txt`)
2. **Preparation**:
   - Frame creation with header
   - AES-256-GCM encryption
   - FEC encoding (3+3 shards)
   - Interleaving and bitstream serialization
   - Preamble injection
3. **Modulation**:
   - Bitstream → symbols → DSSS spreading
   - Frequency bin selection → chip embedding
   - Audio frame processing (STFT → IFFT → overlap-add)
4. **Transmission**: Real-time audio capture and modulation
5. **Recovery**:
   - Audio capture → frame processing → bit extraction
   - FEC decoding → decryption → original file recovery

## Key Technical Considerations

- **Synchronization**: High-entropy 80-bit preamble for robust sync acquisition
- **Error Resilience**: Reed-Solomon FEC (3+3) for error correction
- **Security**: AES-256-GCM with 600,000 iteration PBKDF2 key derivation
- **Audio Stealth**: Embedding in safe frequency bins identified through masking analysis
- **Real-time Processing**: 1024-sample frames with 512-sample hop size
- **DSSS Parameters**: 64-chip spreading sequences with LFSR-based PN generation

## Component Dependencies

- **Core Modules**: embedding → modulator → profiler → receiver
- **Crypto**: keyDerivation.ts → aes.ts
- **FEC**: readSolomon.ts → interleave.ts
- **Audio**: processFrame.ts → fft.js → freqBarkMap.js

## Testing Coverage

The project includes comprehensive tests for all major components:

- Embedding module tests
- Bitstream serialization tests
- FEC encoding/decoding tests
- Modulation and demodulation tests
- Receiver synchronization tests
- End-to-end recovery tests

## Future Enhancements

- Support for multiple audio channels
- Adaptive modulation based on audio quality
- Enhanced error correction codes
- Multi-file support
- Offline processing capabilities
