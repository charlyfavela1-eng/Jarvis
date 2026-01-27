# -*- encoding: utf-8 -*-
"""
Ask AI - Direct access to Jarvis's AI brain
"""
from colorama import Fore
from plugin import plugin, alias


@alias("ai", "chat", "gpt")
@plugin("ask")
class AskAI:
    """
    Ask Jarvis anything directly using AI.
    He knows ball.
    """

    def __call__(self, jarvis, s):
        from utilities.llm import get_llm

        if not s or not s.strip():
            jarvis.say("What do you want to know?", Fore.CYAN)
            s = jarvis.input(Fore.MAGENTA + "You: " + Fore.RESET)

        if not s.strip():
            jarvis.say("Ask me anything!", Fore.CYAN)
            return

        llm = get_llm()
        if llm.is_available():
            # Stream response
            print(Fore.CYAN, end='', flush=True)
            full_response = ""
            for chunk in llm.ask(s, stream=True):
                print(chunk, end='', flush=True)
                full_response += chunk
            print(Fore.RESET)
        else:
            jarvis.say("AI not configured. Set OPENAI_API_KEY env variable.", Fore.RED)
