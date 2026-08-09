# DeepHide System Architecture: Concurrency & Thread Mapping

**Document Purpose:** Architectural blueprint transitioning DeepHide from a single-threaded JavaScript event-loop model to a deterministic, multi-threaded Rust environment for real-time DSP.

---

## 1. The Architectural Shift

### The Legacy Node.js Model (Single-Threaded Bottleneck)

In the previous implementation, the V8 JavaScript engine handled all operations on a single main thread, utilizing an event loop for asynchronous tasks.

- **Audio Capture:** Pushed data to the event loop.
- **DSP Processing:** STFT/IFFT math blocked the main thread.
- **Garbage Collection (GC):** Unpredictable pauses to clear memory.
- **Result:** When CPU-heavy modulation (AES-256-GCM, Reed-Solomon, STFT) executed, incoming 1024-sample audio frames from `PvRecorder` were delayed or dropped[cite: 1, 2].

### The Rust Model (Fearless Concurrency)

Rust allows the physical separation of time-critical I/O from CPU-heavy mathematics. By utilizing OS-level threads and lock-free memory structures, the system guarantees that the audio hardware never waits for the CPU.

| Component        | Node.js (Legacy)                          | Rust (Target)                            |
| :--------------- | :---------------------------------------- | :--------------------------------------- |
| **Execution**    | Single Event Loop                         | OS-Level Multithreading                  |
| **Memory**       | Garbage Collected (Latency Spikes)        | Ownership & Drop (Deterministic)         |
| **Data Sharing** | Shared Memory Space                       | Message Passing / Lock-Free Ring Buffers |
| **Hardware I/O** | `PvRecorder` (Async Callback)[cite: 1, 2] | `cpal` (Dedicated Real-Time Thread)      |

---

## 2. DeepHide Thread Topology

To guarantee zero dropped frames, the system is strictly divided into three isolated execution environments.

### Thread A: The Main Orchestrator (Low Priority)

- **Responsibility:** Setup, teardown, file I/O, and user interface.
- **Tasks:**
  - Reads the input file (e.g., `file.txt`) from disk[cite: 1, 2].
  - Executes the **Data Preparation Module**: framing, AES-256-GCM encryption, Reed-Solomon (3+3) FEC, and interleaving[cite: 1, 2].
  - Generates the final binary payload and preamble[cite: 1, 2].
  - Spawns and monitors Thread B and Thread C.
- **Constraints:** Can block, can allocate memory dynamically, can handle heavy file I/O without affecting the audio stream.

### Thread B: The Audio I/O Thread (Maximum Priority)

- **Responsibility:** Exclusively interacting with the microphone via `cpal`.
- **Tasks:**
  - Wakes up precisely when the audio interface has a new buffer of samples.
  - Immediately copies the raw floating-point samples into the `Producer` side of a lock-free ring buffer.
  - Goes back to sleep.
- **Strict Rules:**
  - **NO** memory allocation (`Vec::new()`, `Box`, `String`).
  - **NO** locking (`Mutex`, `RwLock`).
  - **NO** file I/O or `println!` statements.

### Thread C: The DSP Worker Thread (High Priority / CPU Bound)

- **Responsibility:** Executing the **Audio Processing Pipeline** and **Modulation Layer**[cite: 1, 2].
- **Tasks:**
  - Polls the `Consumer` side of the lock-free ring buffer for data.
  - Accumulates samples until a full 1024-sample frame is reached[cite: 1, 2].
  - Applies the Hamming window[cite: 1, 2].
  - Executes the Forward STFT (in-place)[cite: 1, 2].
  - Embeds the 64-chip DSSS sequences into the safe frequency bins[cite: 1, 2].
  - Executes the IFFT and overlap-add processing (512-sample hop size)[cite: 1, 2].
  - Pushes the modulated audio out to the speaker or file buffer.
- **Strict Rules:** Operates continuously; all arrays and FFT planners must be pre-allocated before the thread begins processing.

---

## 3. Cross-Thread Communication (The Lock-Free Bridge)

To connect Thread B (Hardware) and Thread C (Math) without blocking, DeepHide will utilize a Single-Producer, Single-Consumer (SPSC) Lock-Free Ring Buffer.

### The Mechanism

1.  **Pre-allocation:** Before any audio starts, a contiguous block of memory (e.g., capable of holding 8192 `f32` samples) is allocated on the heap.
2.  **Split Ownership:** The buffer is split into a `Producer` (owned by Thread B) and a `Consumer` (owned by Thread C).
3.  **Data Flow (Embedding):**
    - _Input:_ Thread B writes raw microphone samples to the head of the buffer.
    - _Processing:_ Thread C reads from the tail of the buffer in 1024-sample chunks[cite: 1, 2].
4.  **Why this guarantees safety:** Because the Producer only updates the "write index" and the Consumer only updates the "read index", they never fight for the same memory address. No Mutex is required, meaning Thread B is never delayed by Thread C's STFT calculations.

---

## 4. Documentation & Crate Map

When researching how to implement this specific topology, refer to the following Rust crates and their documentation:

- **Audio Hardware (Thread B):** `cpal` (`docs.rs/cpal`) - Focus on stream building and the constraints of the data callback.
- **Ring Buffers (The Bridge):** `rtrb` (`docs.rs/rtrb`) or `ringbuf` (`docs.rs/ringbuf`) - Focus on real-time safe SPSC queues.
- **DSP Math (Thread C):** `rustfft` (`docs.rs/rustfft`) - Focus on pre-planning the FFT and executing it against pre-allocated, mutable slices (`&mut [Complex<f32>]`).
