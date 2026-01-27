# -*- encoding: utf-8 -*-

import os
from colorama import Fore
import nltk
import re
import sys
import tempfile
from utilities.GeneralUtilities import print_say
from CmdInterpreter import CmdInterpreter
from utilities.llm import get_llm

# register hist path
HISTORY_FILENAME = tempfile.TemporaryFile('w+t')


PROMPT_CHAR = '~>'

"""
    AUTHORS' SCOPE:
        We thought that the source code of Jarvis would
        be more organized if we treat Jarvis as Object.
        So we decided to create this Jarvis Class which
        implements the core functionality of Jarvis in a
        simpler way than the original __main__.py.
    HOW TO EXTEND JARVIS:
        In progress..
    DETECTED ISSUES:
        * Furthermore, "near me" command is unable to find
        the actual location of our laptops.
"""


class Jarvis(CmdInterpreter, object):
    # variable used at Breakpoint #1.
    # allows Jarvis say "Hi", only at the first interaction.
    first_reaction_text = ""
    first_reaction_text += Fore.CYAN + \
        'Jarvis\' sound is by default disabled.' + Fore.RESET
    first_reaction_text += "\n"
    first_reaction_text += Fore.CYAN + 'In order to let Jarvis talk out loud type: '
    first_reaction_text += Fore.RESET + Fore.MAGENTA + 'enable sound' + Fore.RESET
    first_reaction_text += "\n"
    first_reaction_text += Fore.CYAN + \
        "Type 'help' for a list of available actions." + Fore.RESET
    first_reaction_text += "\n"
    prompt = (
        Fore.MAGENTA
        + "{} Hi, what can I do for you?\n".format(PROMPT_CHAR)
        + Fore.RESET)

    # Used to store user specific data

    def __init__(self, first_reaction_text=first_reaction_text,
                 prompt=prompt, first_reaction=True,
                 directories=["jarviscli/plugins", "custom"]):
        directories = self._rel_path_fix(directories)

        if sys.platform == 'win32':
            self.use_rawinput = False
        self.regex_dot = re.compile('\\.(?!\\w)')
        CmdInterpreter.__init__(self, first_reaction_text, prompt,
                                directories, first_reaction)

    def _rel_path_fix(self, dirs):
        dirs_abs = []
        work_dir = os.path.dirname(__file__)
        # remove 'jarviscli/' from path
        work_dir = os.path.dirname(work_dir)

        # fix nltk path
        nltk.data.path.append(os.path.join(work_dir, "jarviscli/data/nltk"))

        # relative -> absolute paths
        for directory in dirs:
            if not directory.startswith(work_dir):
                directory = os.path.join(work_dir, directory)
            dirs_abs.append(directory)
        return dirs_abs

    def default(self, data):
        """
        When no plugin matches, Jarvis uses AI to answer.
        He just knows ball - answers anything instantly.
        """
        # Get the original input (data might be "None" if parsing failed)
        original_input = getattr(self, '_last_raw_input', data)
        if data == "None" or not data.strip():
            original_input = getattr(self, '_last_raw_input', '')

        if not original_input or original_input.strip() == '':
            print_say("I could not identify your command...", self, Fore.MAGENTA)
            return

        # Use LLM to answer
        llm = get_llm()
        if llm.is_available():
            # Stream response for instant feel
            print(Fore.CYAN, end='', flush=True)
            full_response = ""
            for chunk in llm.ask(original_input, stream=True):
                print(chunk, end='', flush=True)
                full_response += chunk
            print(Fore.RESET)

            # Speak the response if voice is enabled
            if self.enable_voice and full_response:
                self.speak(full_response)
        else:
            # Fallback if no API key
            print_say("I don't have an AI brain configured yet.", self, Fore.MAGENTA)
            print_say("Set ANTHROPIC_API_KEY or OPENAI_API_KEY to enable AI.", self, Fore.YELLOW)

    def precmd(self, line):
        """Hook that executes before every command."""
        # Store original input for AI fallback
        self._last_raw_input = line

        words = line.split()
        HISTORY_FILENAME.write(line + '\n')

        # append calculate keyword to front of leading char digit (or '-') in line
        if words and (words[0].isdigit() or line[0] == "-"):
            line = "calculate " + line
            words = line.split()

        if line.startswith("help"):
            return line
        if line.startswith("status"):
            return line

        if not words:
            line = "None"
        else:
            line = self.parse_input(line)
        return line

    def postcmd(self, stop, line):
        """Hook that executes after every command."""
        if self.first_reaction:
            self.prompt = (
                Fore.MAGENTA
                + "{} What can I do for you?\n".format(PROMPT_CHAR)
                + Fore.RESET)
            self.first_reaction = False
        if self.enable_voice:
            self.speech.text_to_speech("What can I do for you?\n")

    def speak(self, text):
        if self.enable_voice:
            self.speech.text_to_speech(text)

    def parse_input(self, data):
        """This method gets the data and assigns it to an action"""
        data = data.lower()
        # say command is better if data has punctuation marks
        if "say" not in data:
            data = data.replace("?", "")
            data = data.replace("!", "")
            data = data.replace(",", "")

            # input sanitisation to not mess up urls / numbers
            data = self.regex_dot.sub("", data)

        # Check if Jarvis has a fixed response to this data
        if data in self.fixed_responses:
            output = self.fixed_responses[data]
        else:
            # if it doesn't have a fixed response, look if the data corresponds
            # to an action
            output = self.find_action(
                data, self._plugin_manager.get_plugins().keys())
        return output

    def find_action(self, data, actions):
        """Checks if input is a defined action.
        :return: returns the action"""
        output = "None"
        if not actions:
            return output

        action_found = False
        words = data.split()
        actions = list(actions)

        # return longest matching word
        # TODO: Implement real and good natural language processing
        # But for now, this code returns acceptable results
        actions.sort(key=lambda l: len(l), reverse=True)

        # check word by word if exists an action with the same name
        for action in actions:
            words_remaining = data.split()
            for word in words:
                words_remaining.remove(word)
                # For the 'near' keyword, the words before 'near' are also needed
                if word == "near":
                    initial_words = words[:words.index('near')]
                    output = word + " " +\
                        " ".join(initial_words + ["|"] + words_remaining)
                elif word == action:  # command name exists
                    action_found = True
                    output = word + " " + " ".join(words_remaining)
                    break
            if action_found:
                break
        return output

    def executor(self, command):
        """
        If command is not empty, we execute it and terminate.
        Else, this method opens a terminal session with the user.
        We can say that it is the core function of this whole class
        and it joins all the function above to work together like a
        clockwork. (Terminates when the user send the "exit", "quit"
        or "goodbye command")
        :return: Nothing to return.
        """
        if command:
            self.execute_once(command)
        else:
            self.cmdloop()
