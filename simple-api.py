import asyncio
import os
import sys
import base64
import json
import aiohttp
import numpy as np
import pyaudio
from dotenv import load_dotenv

load_dotenv(override=True)


class VibeVoiceTTS:
    """Simple VibeVoice API client with SSE streaming"""

    def __init__(self, api_url: str = "http://127.0.0.1:8000/v1"):
        self.api_url = api_url
        self.voice_path = os.getenv("PATH_TO_VOICE_FILE", "primavera.wav")
        self.model = os.getenv("VIBEVOICE_MODEL_PATH", "vibevoice/VibeVoice-7B")
        self.ddpm_steps = int(os.getenv("DDPM_STEPS", 20))
        self.cfg_scale = float(os.getenv("CFG_SCALE", 3.0))

    async def synthesize(self, text: str) -> tuple[np.ndarray, int]:
        """Generate audio from text, returns (audio_array, sample_rate)"""
        payload = {
            "model": self.model,
            "voice": "primavera",
            "input": text,
            "response_format": "pcm",
            "voice_path": self.voice_path,
            "stream_format": "sse",
            "ddpm_steps": self.ddpm_steps,
            "cfg_scale": self.cfg_scale,
        }

        audio_chunks = []
        sample_rate = 24000

        async with aiohttp.ClientSession() as session:
            async with session.post(
                f"{self.api_url}/audio/speech",
                json=payload,
                headers={"Content-Type": "application/json"}
            ) as response:
                if response.status != 200:
                    print(f"API error: {response.status}", file=sys.stderr)
                    return np.array([], dtype=np.int16), sample_rate

                current_event = None
                data_buffer = ""

                async for line_bytes in response.content:
                    line = line_bytes.decode('utf-8').strip()

                    if not line:
                        if current_event and data_buffer:
                            try:
                                if current_event == 'start':
                                    meta = json.loads(data_buffer)
                                    sample_rate = int(meta.get("sample_rate", 24000))

                                elif current_event == 'chunk':
                                    payload_data = json.loads(data_buffer)
                                    audio_b64 = payload_data.get("data")
                                    if audio_b64:
                                        audio_bytes = base64.b64decode(audio_b64)
                                        audio_np = np.frombuffer(audio_bytes, dtype=np.int16)
                                        audio_chunks.append(audio_np)

                                elif current_event == 'end':
                                    break

                                elif current_event == 'error':
                                    error_data = json.loads(data_buffer)
                                    print(f"API error: {error_data}", file=sys.stderr)
                                    break

                            except json.JSONDecodeError as e:
                                print(f"JSON decode error: {e}", file=sys.stderr)

                        current_event, data_buffer = None, ""
                        continue

                    if line.startswith('event:'):
                        current_event = line[len('event:'):].strip()
                    elif line.startswith('data:'):
                        data_buffer += line[len('data:'):].strip()

        if audio_chunks:
            return np.concatenate(audio_chunks), sample_rate
        return np.array([], dtype=np.int16), sample_rate


def play_audio(audio: np.ndarray, sample_rate: int = 24000):
    """Play audio array through speakers"""
    if len(audio) == 0:
        return

    p = pyaudio.PyAudio()
    stream = p.open(
        format=pyaudio.paInt16,
        channels=1,
        rate=sample_rate,
        output=True
    )
    stream.write(audio.tobytes())
    stream.stop_stream()
    stream.close()
    p.terminate()


def save_audio(audio: np.ndarray, sample_rate: int = 24000, filename: str = "output.wav"):
    """Save audio array to WAV file"""
    import wave
    if len(audio) == 0:
        return

    with wave.open(filename, 'wb') as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)  # 16-bit
        wf.setframerate(sample_rate)
        wf.writeframes(audio.tobytes())
    print(f"Saved audio to {filename}")


async def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} [--save output.wav] <text to speak>")
        sys.exit(1)

    # Check for --save flag
    save_file = None
    args = sys.argv[1:]
    if args[0] == "--save" and len(args) > 1:
        save_file = args[1]
        args = args[2:]

    if not args:
        print(f"Usage: {sys.argv[0]} [--save output.wav] <text to speak>")
        sys.exit(1)

    text = " ".join(args)
    api_url = os.getenv("VIBEVOICE_API_URL", "http://127.0.0.1:8000/v1")

    tts = VibeVoiceTTS(api_url=api_url)
    audio, sample_rate = await tts.synthesize(text)

    if len(audio) > 0:
        if save_file:
            save_audio(audio, sample_rate, save_file)
        else:
            play_audio(audio, sample_rate)
    else:
        print("No audio generated", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())