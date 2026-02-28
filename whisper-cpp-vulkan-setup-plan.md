# Claude Code Plan: Install & Setup whisper.cpp-vulkan + base model

## Target System
- **OS:** CachyOS (Arch-based)
- **GPU:** Intel Arc Graphics (Vulkan)
- **DE:** KDE Plasma 6.5 on Wayland
- **Package Manager:** pacman + paru (AUR)

---

## Phase 1: Verify Prerequisites

```bash
# 1.1 - Confirm Vulkan is working with Intel Arc
vulkaninfo --summary 2>/dev/null | head -20

# 1.2 - Check Intel Vulkan driver is installed
pacman -Qs vulkan-intel

# 1.3 - If missing, install Vulkan stack
sudo pacman -S --needed vulkan-intel vulkan-icd-loader vulkan-headers

# 1.4 - Confirm build tools are present
sudo pacman -S --needed cmake git base-devel sdl2 openblas
```

**Checkpoint:** `vulkaninfo --summary` should show your Intel Arc device. If it doesn't, stop and troubleshoot the GPU driver first.

---

## Phase 2: Install whisper.cpp with Vulkan Support

### Option A: AUR Package (Preferred)

```bash
# 2.1 - Install from AUR with Vulkan support
paru -S whisper.cpp

# 2.2 - Verify it was built with Vulkan
whisper-cli --help 2>&1 | head -5
```

**Note:** The current AUR `whisper.cpp` PKGBUILD includes `-DGGML_VULKAN=1`. If the AUR package has issues or you want guaranteed Vulkan, fall back to Option B.

### Option B: Build from Source (Fallback)

```bash
# 2.1 - Clone repo
cd ~/src  # or wherever you keep source builds
git clone https://github.com/ggml-org/whisper.cpp.git
cd whisper.cpp

# 2.2 - Configure with Vulkan + SDL2 (for mic input)
cmake -B build \
  -DGGML_VULKAN=1 \
  -DWHISPER_SDL2=1 \
  -DCMAKE_BUILD_TYPE=Release

# 2.3 - Build (use all cores)
cmake --build build -j$(nproc)

# 2.4 - Optionally install system-wide
sudo cmake --install build --prefix /usr/local

# 2.5 - Verify binary exists
./build/bin/whisper-cli --help 2>&1 | head -5
```

---

## Phase 3: Download the Base Model

```bash
# 3.1 - If installed via AUR, the download script should be available as:
whisper-cpp-download-ggml-model base

# 3.2 - If built from source, use the bundled script:
cd ~/src/whisper.cpp
bash models/download-ggml-model.sh base

# 3.3 - Verify model file exists and is ~148MB
ls -lh models/ggml-base.bin
# Expected: approximately 148M
```

**Model location notes:**
- AUR install: models typically go to `~/.local/share/whisper.cpp/models/` or `/usr/share/whisper.cpp/models/`
- Source build: `~/src/whisper.cpp/models/ggml-base.bin`

Store the actual model path in a variable for subsequent steps:

```bash
WHISPER_MODEL="$(find /usr ~/.local ~/src -name 'ggml-base.bin' 2>/dev/null | head -1)"
echo "Model at: $WHISPER_MODEL"
```

---

## Phase 4: Test — Offline File Transcription

```bash
# 4.1 - Download or use the bundled JFK sample
#        (bundled with source build at samples/jfk.wav)
#        If AUR install, grab a test file:
if [ ! -f /tmp/test-whisper.wav ]; then
  curl -L -o /tmp/test-whisper.wav \
    "https://github.com/ggml-org/whisper.cpp/raw/master/samples/jfk.wav"
fi

# 4.2 - Run transcription and confirm Vulkan is used
whisper-cli -m "$WHISPER_MODEL" -f /tmp/test-whisper.wav 2>&1

# Look for output like:
#   ggml_vulkan: Found 1 Vulkan devices:
#   ggml_vulkan: 0 = Intel(R) Arc(TM) ...
#   [00:00:00.000 --> 00:00:11.000]   And so my fellow Americans...
```

**Checkpoint:** The output MUST show `ggml_vulkan` lines referencing your Intel Arc GPU. If it says "no Vulkan devices" or falls back to CPU, the Vulkan build flag wasn't applied — rebuild with Option B.

---

## Phase 5: Test — Real-time Microphone Transcription

```bash
# 5.1 - Check mic is working (PipeWire/PulseAudio)
pactl list sources short

# 5.2 - Run real-time stream mode
#        --step 3000  = process every 3 seconds
#        --length 10000 = keep 10 seconds of context
whisper-stream -m "$WHISPER_MODEL" \
  --step 3000 \
  --length 10000 \
  -t 4

# If installed from source:
# ./build/bin/whisper-stream -m "$WHISPER_MODEL" --step 3000 --length 10000 -t 4
```

**Checkpoint:** Speak into your mic. You should see text appearing in real-time. Press Ctrl+C to stop.

**Troubleshooting:**
- If "failed to open audio device" → check `pactl list sources short` and set default source
- If very slow → confirm Vulkan is active (check stderr for `ggml_vulkan` lines)
- If garbled text → try `--step 4000 --length 12000` for longer context windows

---

## Phase 6: Test — Dictation to File (Practical Use)

```bash
# 6.1 - Record from mic and save transcription to a text file
whisper-stream -m "$WHISPER_MODEL" \
  --step 3000 \
  --length 10000 \
  -t 4 \
  2>/dev/null | tee ~/whisper-test-output.txt

# 6.2 - Review output
cat ~/whisper-test-output.txt
```

---

## Phase 7: Create Convenience Alias / Script

```bash
# 7.1 - Add alias to shell config
cat >> ~/.bashrc << 'EOF'

# whisper.cpp voice dictation
alias dictate='whisper-stream -m "$HOME/.local/share/whisper-models/ggml-base.bin" --step 3000 --length 10000 -t 4'
EOF

# 7.2 - Or create a standalone script
cat > ~/.local/bin/whisper-dictate << 'SCRIPT'
#!/bin/bash
# Quick voice dictation using whisper.cpp with Vulkan
MODEL="${WHISPER_MODEL:-$HOME/.local/share/whisper-models/ggml-base.bin}"

# Find model if not set
if [ ! -f "$MODEL" ]; then
  MODEL="$(find /usr ~/.local ~/src -name 'ggml-base.bin' 2>/dev/null | head -1)"
fi

if [ ! -f "$MODEL" ]; then
  echo "Error: Could not find ggml-base.bin model"
  echo "Run: whisper-cpp-download-ggml-model base"
  exit 1
fi

echo "Using model: $MODEL"
echo "Speak into your mic. Ctrl+C to stop."
echo "---"

exec whisper-stream -m "$MODEL" \
  --step 3000 \
  --length 10000 \
  -t 4 \
  "$@"
SCRIPT
chmod +x ~/.local/bin/whisper-dictate
```

---

## Verification Summary

After completing all phases, confirm:

| Check | Expected Result |
|-------|----------------|
| `vulkaninfo --summary` | Shows Intel Arc device |
| `whisper-cli --help` | Binary runs, shows options |
| `ls -lh $WHISPER_MODEL` | ~148MB ggml-base.bin exists |
| Transcribe JFK sample | Output shows `ggml_vulkan: Intel Arc` + correct text |
| `whisper-stream` with mic | Real-time text from speech |
| `whisper-dictate` script | Convenience wrapper works |

---

## Notes for Future Integration

- **AppMesh/D-Bus:** whisper-stream stdout can be piped or parsed by a D-Bus service to inject text into focused windows via `wtype` (Wayland)
- **PHP/Laravel:** `exec('whisper-cli -m $model -f $audioFile')` works for file-based transcription from a web app
- **Upgrade to small model:** Just run `download-ggml-model.sh small` and change the model path — no rebuild needed
- **Server mode:** whisper.cpp includes a `whisper-server` binary that exposes an HTTP API compatible with OpenAI's `/v1/audio/transcriptions` endpoint
