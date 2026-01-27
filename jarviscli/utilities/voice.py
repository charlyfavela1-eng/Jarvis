import re
import os
import subprocess
import tempfile

from utilities.GeneralUtilities import IS_MACOS, IS_WIN

# Try ElevenLabs for movie-quality JARVIS voice
try:
    from elevenlabs import ElevenLabs
    from elevenlabs import play, Voice, VoiceSettings
    ELEVENLABS_AVAILABLE = True
except ImportError:
    ELEVENLABS_AVAILABLE = False

# Try pydub for audio playback
try:
    from pydub import AudioSegment, playback
    FNULL = open(os.devnull, 'w')
    _subprocess_call = playback.subprocess.call
    playback.subprocess.call = lambda cmd: _subprocess_call(cmd, stdout=FNULL, stderr=subprocess.STDOUT)
    PYDUB_AVAILABLE = True
except ImportError:
    PYDUB_AVAILABLE = False

# Try gTTS
try:
    from gtts import gTTS
    GTTS_AVAILABLE = True
except ImportError:
    GTTS_AVAILABLE = False

if IS_MACOS:
    from os import system
else:
    try:
        import pyttsx3
    except ImportError:
        pass


def remove_ansi_escape_seq(text):
    """Remove ANSI escape sequences from text."""
    if text:
        text = re.sub(r'''(\x9B|\x1B\[)[0-?]*[ -\/]*[@-~]''', '', text)
    return text


def create_voice(self, gtts_status, rate=180):
    """
    Create the best available voice engine.
    Priority: ElevenLabs (movie-quality) > macOS Daniel > GTTS > pyttsx3
    """
    # Try ElevenLabs first - sounds like the real JARVIS
    elevenlabs_key = os.environ.get("ELEVENLABS_API_KEY")
    if ELEVENLABS_AVAILABLE and elevenlabs_key:
        try:
            return VoiceElevenLabs(api_key=elevenlabs_key)
        except Exception:
            pass

    # Fall back to platform-specific
    if IS_MACOS:
        return VoiceMac(voice="Daniel", rate=rate)
    elif IS_WIN:
        return VoiceWin(rate)
    else:
        try:
            return VoiceLinux(rate)
        except Exception:
            return VoiceNotSupported()


class VoiceElevenLabs:
    """
    ElevenLabs voice - Movie-quality JARVIS voice.
    Uses "Daniel" - British, formal, steady broadcaster. Just like Paul Bettany.
    """

    # Daniel - British accent, formal, steady broadcaster - perfect JARVIS voice
    JARVIS_VOICE_ID = "onwK4e9ZLuTAKqWW03F9"  # Daniel (British)

    def __init__(self, api_key=None):
        self.api_key = api_key or os.environ.get("ELEVENLABS_API_KEY")
        self.client = ElevenLabs(api_key=self.api_key)
        self.voice_settings = VoiceSettings(
            stability=0.8,         # Very stable, consistent - like JARVIS
            similarity_boost=0.8,  # Sound like the voice
            style=0.3,             # Subtle expressiveness - calm and collected
            use_speaker_boost=True
        )

    def text_to_speech(self, speech):
        """Convert text to speech using ElevenLabs."""
        speech = remove_ansi_escape_seq(speech)
        if not speech or not speech.strip():
            return

        try:
            audio = self.client.text_to_speech.convert(
                text=speech,
                voice_id=self.JARVIS_VOICE_ID,
                model_id="eleven_turbo_v2_5",  # Fast, high quality
                voice_settings=self.voice_settings
            )

            # Save and play
            with tempfile.NamedTemporaryFile(suffix='.mp3', delete=False) as f:
                for chunk in audio:
                    f.write(chunk)
                temp_path = f.name

            # Play audio
            if IS_MACOS:
                os.system(f'afplay "{temp_path}" 2>/dev/null')
            elif PYDUB_AVAILABLE:
                audio_seg = AudioSegment.from_mp3(temp_path)
                playback.play(audio_seg)
            else:
                os.system(f'mpv --no-video "{temp_path}" 2>/dev/null || ffplay -nodisp -autoexit "{temp_path}" 2>/dev/null')

            os.remove(temp_path)
        except Exception as e:
            print(f"ElevenLabs error: {e}")


class VoiceMac:
    """macOS voice using 'say' command with Daniel (British) voice."""

    def __init__(self, voice="Daniel", rate=180):
        self.voice = voice
        self.rate = rate

    def text_to_speech(self, speech):
        speech = remove_ansi_escape_seq(speech)
        if not speech:
            return
        speech = speech.replace("'", "\\'")
        speech = speech.replace('"', '\\"')
        system(f'say -v {self.voice} -r {self.rate} $\'{speech}\'')


class VoiceGTTS:
    """Google Text-to-Speech."""

    def text_to_speech(self, speech):
        speech = remove_ansi_escape_seq(speech)
        if not speech:
            return
        tts = gTTS(speech, lang="en")
        tts.save("voice.mp3")
        if PYDUB_AVAILABLE:
            audio = AudioSegment.from_mp3('voice.mp3')
            playback.play(audio)
        os.remove("voice.mp3")


class Voice_general:
    def __init__(self, rate):
        self.rate = rate
        self.min_rate = 50
        self.max_rate = 500
        self.create()

    def create(self):
        self.engine = pyttsx3.init()
        self.engine.setProperty('rate', self.rate)

    def destroy(self):
        del self.engine


class VoiceLinux(Voice_general):
    def __init__(self, rate):
        super().__init__(rate)

    def text_to_speech(self, speech):
        if speech != '':
            speech = remove_ansi_escape_seq(speech)
            self.create()
            self.engine.say(speech)
            self.engine.runAndWait()
            self.destroy()

    def change_rate(self, delta):
        if self.rate + delta > self.max_rate:
            self.rate = self.max_rate
        elif self.rate + delta < self.min_rate:
            self.rate = self.min_rate
        else:
            self.rate = self.rate + delta


class VoiceWin:
    def __init__(self, rate):
        self.rate = rate
        self.min_rate = 50
        self.max_rate = 500
        self.create()

    def create(self):
        self.engine = pyttsx3.init()
        self.engine.setProperty('rate', self.rate)

    def destroy(self):
        del self.engine

    def text_to_speech(self, speech):
        speech = remove_ansi_escape_seq(speech)
        self.create()
        self.engine.setProperty('rate', 170)
        voices = self.engine.getProperty('voices')
        self.engine.setProperty('voices', voices[1].id)
        self.engine.say(speech)
        self.engine.runAndWait()
        self.destroy()

    def change_rate(self, delta):
        if self.rate + delta > self.max_rate:
            self.rate = self.max_rate
        elif self.rate + delta < self.min_rate:
            self.rate = self.min_rate
        else:
            self.rate = self.rate + delta


class VoiceNotSupported:
    def __init__(self):
        self.warning_print = False

    def text_to_speech(self, speech):
        if not self.warning_print:
            print("Speech not supported! Install elevenlabs or pyttsx3.")
            self.warning_print = True
