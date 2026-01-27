#!/usr/bin/env python3
"""
JARVIS Voice Listener - Always listening, activates on wake word
Wake words: "Jarvis", "Hey Jarvis"
Special: "Daddy's home" plays Back in Black and activates JARVIS
"""

import os
import sys
import time
import tempfile
import threading
import subprocess

# Add Jarvis to path
sys.path.insert(0, '/Users/michaelcaneyjr/Jarvis/jarviscli')
os.chdir('/Users/michaelcaneyjr/Jarvis')

# Load environment
ELEVENLABS_API_KEY = os.environ.get('ELEVENLABS_API_KEY', 'sk_9dd7ba5d73c9b50c8a37b295f5b0d872f0a6ebb4c24bf7eb')
SHODAN_API_KEY = os.environ.get('SHODAN_API_KEY', 'uJSeUuGvN4aEhVwCJzDMEQimOD8jZLkJ')

# Try imports
try:
    import speech_recognition as sr
    SR_AVAILABLE = True
except ImportError:
    SR_AVAILABLE = False
    print("Install speech_recognition: pip install SpeechRecognition")

try:
    from elevenlabs import ElevenLabs, VoiceSettings
    ELEVENLABS_AVAILABLE = True
except ImportError:
    ELEVENLABS_AVAILABLE = False

# Import JARVIS LLM
try:
    from utilities.llm import get_llm
    LLM_AVAILABLE = True
except ImportError:
    LLM_AVAILABLE = False
    print("Could not import LLM utilities")


class JarvisListener:
    """Always-on JARVIS listener with wake word detection."""

    WAKE_WORDS = ['jarvis', 'hey jarvis', 'yo jarvis', 'ok jarvis']
    SPECIAL_WAKE = ["daddy's home", "daddys home", "daddy is home", "wake up daddys home", "wake up daddy's home"]

    # British Daniel voice on ElevenLabs - THE ONLY JARVIS VOICE
    VOICE_ID = 'onwK4e9ZLuTAKqWW03F9'

    def __init__(self):
        self.recognizer = sr.Recognizer()
        self.microphone = sr.Microphone()
        self.running = False
        self.llm = get_llm() if LLM_AVAILABLE else None

        # Adjust for ambient noise
        print("JARVIS initializing... calibrating microphone...")
        with self.microphone as source:
            self.recognizer.adjust_for_ambient_noise(source, duration=2)

        # ElevenLabs client - PRIMARY JARVIS VOICE
        self.voice_client = None
        if ELEVENLABS_AVAILABLE and ELEVENLABS_API_KEY:
            self.voice_client = ElevenLabs(api_key=ELEVENLABS_API_KEY)
            print("ElevenLabs JARVIS voice loaded.")
        else:
            print("WARNING: ElevenLabs not available, using macOS fallback")

        print("JARVIS ready. Listening for wake word...")

    def speak(self, text):
        """Speak text using ElevenLabs JARVIS voice (British Daniel)."""
        if not text:
            return

        # ElevenLabs - THE ONLY JARVIS VOICE
        if self.voice_client:
            try:
                audio = self.voice_client.text_to_speech.convert(
                    text=text,
                    voice_id=self.VOICE_ID,
                    model_id='eleven_turbo_v2_5',
                    voice_settings=VoiceSettings(
                        stability=0.8,
                        similarity_boost=0.8,
                        style=0.3,
                        use_speaker_boost=True
                    )
                )

                with tempfile.NamedTemporaryFile(suffix='.mp3', delete=False) as f:
                    for chunk in audio:
                        f.write(chunk)
                    temp_path = f.name

                os.system(f'afplay "{temp_path}" 2>/dev/null')
                os.remove(temp_path)
                return
            except Exception as e:
                print(f"ElevenLabs error: {e}")

        # Emergency fallback to macOS (only if ElevenLabs completely fails)
        text_escaped = text.replace('"', '\\"')
        os.system(f'say -v Daniel "{text_escaped}"')

    def play_back_in_black(self):
        """Play Back in Black intro for daddy's home."""
        # Try to play Back in Black
        back_in_black_paths = [
            '/Users/michaelcaneyjr/Music/back_in_black.mp3',
            '/Users/michaelcaneyjr/Jarvis/sounds/back_in_black.mp3',
            os.path.expanduser('~/Music/back_in_black.mp3'),
        ]

        for path in back_in_black_paths:
            if os.path.exists(path):
                # Play just the intro (first 10 seconds)
                os.system(f'afplay "{path}" &')
                time.sleep(8)
                os.system('pkill -f afplay')
                return True

        # If no file found, just play a notification sound
        os.system('afplay /System/Library/Sounds/Glass.aiff')
        return False

    def daddys_home(self):
        """Special activation sequence for daddy's home."""
        print("\n🎸 DADDY'S HOME!")
        self.play_back_in_black()
        self.speak("Welcome home, Sir. All systems are online and awaiting your command. I've prepared your usual briefing. Shall I proceed?")

    def process_command(self, command):
        """Process a voice command through JARVIS AI."""
        if not command:
            return

        print(f"\n📝 Command: {command}")

        if not self.llm or not self.llm.is_available():
            self.speak("I'm sorry Sir, my neural networks are not available at the moment.")
            return

        # Get response from LLM
        response = self.llm.ask(command, stream=False)

        if response:
            print(f"🤖 JARVIS: {response[:200]}...")
            self.speak(response)
        else:
            self.speak("I apologize Sir, I couldn't process that request.")

    def listen_for_command(self):
        """Listen for a command after wake word detected."""
        self.speak("Yes, Sir?")

        print("🎤 Listening for command...")

        try:
            with self.microphone as source:
                audio = self.recognizer.listen(source, timeout=10, phrase_time_limit=30)

            command = self.recognizer.recognize_google(audio).lower()
            return command
        except sr.WaitTimeoutError:
            self.speak("I didn't catch that, Sir.")
            return None
        except sr.UnknownValueError:
            self.speak("I couldn't understand that, Sir.")
            return None
        except Exception as e:
            print(f"Error: {e}")
            return None

    def listen_loop(self):
        """Main listening loop - always on."""
        self.running = True

        while self.running:
            try:
                with self.microphone as source:
                    print("\r👂 Listening...", end='', flush=True)
                    audio = self.recognizer.listen(source, timeout=None, phrase_time_limit=5)

                # Try to recognize wake word
                try:
                    text = self.recognizer.recognize_google(audio).lower()
                    print(f"\r   Heard: {text[:50]}...", end='', flush=True)

                    # Check for special wake word
                    for special in self.SPECIAL_WAKE:
                        if special in text:
                            self.daddys_home()
                            command = self.listen_for_command()
                            if command:
                                self.process_command(command)
                            break
                    else:
                        # Check for normal wake word
                        for wake in self.WAKE_WORDS:
                            if wake in text:
                                # Check if command is in the same phrase
                                after_wake = text.split(wake, 1)[-1].strip()
                                if after_wake and len(after_wake) > 3:
                                    # Command included with wake word
                                    self.process_command(after_wake)
                                else:
                                    # Just wake word, listen for command
                                    command = self.listen_for_command()
                                    if command:
                                        self.process_command(command)
                                break

                except sr.UnknownValueError:
                    # Didn't understand - keep listening
                    pass
                except sr.RequestError as e:
                    print(f"\nSpeech recognition error: {e}")
                    time.sleep(1)

            except KeyboardInterrupt:
                print("\n\nJARVIS shutting down...")
                self.running = False
                break
            except Exception as e:
                print(f"\nError: {e}")
                time.sleep(1)

    def start(self):
        """Start JARVIS listener."""
        self.speak("JARVIS online. Standing by, Sir.")
        self.listen_loop()

    def stop(self):
        """Stop JARVIS listener."""
        self.running = False
        self.speak("JARVIS going offline. Goodbye, Sir.")


def main():
    if not SR_AVAILABLE:
        print("Error: speech_recognition not installed")
        print("Run: pip install SpeechRecognition pyaudio")
        sys.exit(1)

    print("""
    ╔═══════════════════════════════════════════╗
    ║           J.A.R.V.I.S. LISTENER           ║
    ║   Just A Rather Very Intelligent System   ║
    ╠═══════════════════════════════════════════╣
    ║  Wake words: "Jarvis", "Hey Jarvis"       ║
    ║  Special: "Daddy's home" (Back in Black)  ║
    ║  Press Ctrl+C to quit                     ║
    ╚═══════════════════════════════════════════╝
    """)

    jarvis = JarvisListener()
    jarvis.start()


if __name__ == '__main__':
    main()
