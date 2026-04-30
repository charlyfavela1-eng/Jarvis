import os

import anthropic
from plugin import plugin, require


SYSTEM_PROMPT = (
    "You are Jarvis, a helpful AI assistant integrated into the Jarvis CLI. "
    "You are concise, accurate, and friendly. Provide clear and direct responses. "
    "When performing tasks, explain your approach and give relevant information."
)


@require(network=True)
@plugin('claude')
class ClaudeAI:
    """
    Chat with Claude AI (powered by Anthropic)

    Requires the ANTHROPIC_API_KEY environment variable to be set.

    Usage:
      claude          - Start a multi-turn conversation
      claude <text>   - Send a single message and get a response

    During conversation:
      exit / quit     - End the conversation
      clear           - Reset conversation history
      help            - Show usage information

    -- Example:
      claude
      claude What is the capital of France?
      claude Explain quantum computing in simple terms
    """

    def __call__(self, jarvis, s):
        if not os.environ.get("ANTHROPIC_API_KEY"):
            jarvis.say(
                "Error: ANTHROPIC_API_KEY is not set.\n"
                "Export your API key before using Claude:\n"
                "  export ANTHROPIC_API_KEY='your-key-here'"
            )
            return

        if s:
            self._ask_claude(jarvis, s.strip(), history=[])
        else:
            self._conversation_loop(jarvis)

    def _conversation_loop(self, jarvis):
        jarvis.say("Claude AI ready. Commands: 'exit', 'clear', 'help'")
        history = []

        while True:
            try:
                user_input = jarvis.input("You: ").strip()
            except (KeyboardInterrupt, EOFError):
                jarvis.say("\nGoodbye!")
                break

            if not user_input:
                continue

            cmd = user_input.lower()

            if cmd in ("exit", "quit"):
                jarvis.say("Goodbye!")
                break

            if cmd == "clear":
                history = []
                jarvis.say("Conversation history cleared.")
                continue

            if cmd == "help":
                jarvis.say(
                    "Commands:\n"
                    "  exit / quit  End conversation\n"
                    "  clear        Reset history\n"
                    "  help         Show this message"
                )
                continue

            self._ask_claude(jarvis, user_input, history)

    def _ask_claude(self, jarvis, user_message, history):
        client = anthropic.Anthropic()
        messages = list(history) + [{"role": "user", "content": user_message}]

        try:
            print("Claude: ", end="", flush=True)
            full_response = ""

            with client.messages.stream(
                model="claude-opus-4-7",
                max_tokens=16000,
                system=[{
                    "type": "text",
                    "text": SYSTEM_PROMPT,
                    "cache_control": {"type": "ephemeral"},
                }],
                messages=messages,
            ) as stream:
                for text in stream.text_stream:
                    print(text, end="", flush=True)
                    full_response += text

            print()

            history.append({"role": "user", "content": user_message})
            history.append({"role": "assistant", "content": full_response})

        except anthropic.AuthenticationError:
            jarvis.say(
                "Authentication failed. Check your ANTHROPIC_API_KEY."
            )
        except anthropic.RateLimitError:
            jarvis.say("Rate limit reached. Please wait and try again.")
        except anthropic.APIConnectionError:
            jarvis.say("Network error. Check your internet connection.")
        except anthropic.APIStatusError as e:
            jarvis.say(f"API error {e.status_code}: {e.message}")
