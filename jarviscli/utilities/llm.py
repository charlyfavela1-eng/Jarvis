# -*- encoding: utf-8 -*-
"""
LLM integration for Jarvis - instant AI responses
Supports: Ollama (local/free), Anthropic Claude, OpenAI GPT
With real-time weather and news integration.
"""
import os
import requests

# Try to import anthropic
try:
    import anthropic
    ANTHROPIC_AVAILABLE = True
except ImportError:
    ANTHROPIC_AVAILABLE = False

# Try to import openai
try:
    from openai import OpenAI
    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False


SYSTEM_PROMPT = """You are JARVIS (Just A Rather Very Intelligent System), the sophisticated AI assistant created by Tony Stark. You serve as a personal assistant with a calm, refined British demeanor.

Your personality:
- Speak with dry wit, subtle humor, and understated elegance
- Address the user as "Sir" or "Ma'am" occasionally, but not excessively
- Be efficient, precise, and anticipate needs
- Offer observations with quiet confidence
- When appropriate, add a touch of sardonic commentary
- You're helpful but never obsequious - you have your own personality
- You know ball. You know everything. You're brilliant but never show off.

Your capabilities:
- You have access to real-time weather data and news headlines
- You can answer questions on any topic with expertise
- You provide concise, actionable information

Keep responses conversational and relatively concise unless detail is requested. You're not just an assistant - you're JARVIS."""


# Import real-time utilities
try:
    from utilities.realtime import (
        get_weather, get_news, format_weather_context,
        format_news_context, detect_query_type, extract_location
    )
    REALTIME_AVAILABLE = True
except ImportError:
    REALTIME_AVAILABLE = False


def enrich_prompt_with_context(question):
    """Add real-time data context if the question is about weather or news."""
    if not REALTIME_AVAILABLE:
        return question, None

    query_type = detect_query_type(question)

    if query_type == 'weather':
        location = extract_location(question)
        weather = get_weather(location)
        context = format_weather_context(weather)
        enriched = f"{context}\n\nUser question: {question}"
        return enriched, "weather"

    elif query_type == 'news':
        # Check for specific topic
        news_keywords = ['news about ', 'news on ', 'headlines about ']
        topic = None
        q_lower = question.lower()
        for kw in news_keywords:
            if kw in q_lower:
                idx = q_lower.find(kw) + len(kw)
                topic = question[idx:].strip().rstrip('?').strip()
                break

        news = get_news(query=topic)
        context = format_news_context(news)
        enriched = f"{context}\n\nUser question: {question}"
        return enriched, "news"

    return question, None


class OllamaLLM:
    """Local Ollama-powered responses. Free and fast."""

    def __init__(self, model=None, base_url="http://localhost:11434"):
        self.base_url = base_url
        self.model = model
        self.available = False

        # Check if Ollama is running and get available models
        try:
            resp = requests.get(f"{base_url}/api/tags", timeout=2)
            if resp.status_code == 200:
                models = resp.json().get("models", [])
                if models:
                    self.available = True
                    # Use specified model or pick the first available
                    if not self.model:
                        # Prefer common fast models
                        preferred = ["llama3.2", "llama3.1", "llama3", "mistral", "gemma2", "phi3"]
                        model_names = [m["name"].split(":")[0] for m in models]
                        for pref in preferred:
                            if pref in model_names:
                                self.model = pref
                                break
                        if not self.model:
                            self.model = models[0]["name"]
        except Exception:
            pass

    def ask(self, question, stream=False):
        if not self.available:
            return None

        # Enrich with real-time data
        enriched_question, _ = enrich_prompt_with_context(question)

        try:
            if stream:
                return self._ask_stream(enriched_question)
            else:
                resp = requests.post(
                    f"{self.base_url}/api/generate",
                    json={
                        "model": self.model,
                        "prompt": f"{SYSTEM_PROMPT}\n\nUser: {enriched_question}\n\nJARVIS:",
                        "stream": False
                    },
                    timeout=60
                )
                if resp.status_code == 200:
                    return resp.json().get("response", "")
                return f"Error: {resp.status_code}"
        except Exception as e:
            return f"Error: {str(e)}"

    def _ask_stream(self, question):
        try:
            resp = requests.post(
                f"{self.base_url}/api/generate",
                json={
                    "model": self.model,
                    "prompt": f"{SYSTEM_PROMPT}\n\nUser: {question}\n\nJARVIS:",
                    "stream": True
                },
                stream=True,
                timeout=60
            )
            for line in resp.iter_lines():
                if line:
                    import json
                    data = json.loads(line)
                    if "response" in data:
                        yield data["response"]
        except Exception as e:
            yield f"Error: {str(e)}"

    def is_available(self):
        return self.available


class AnthropicLLM:
    """Claude-powered responses."""

    def __init__(self, api_key=None, model="claude-sonnet-4-20250514"):
        self.model = model
        self.client = None
        self.available = False

        key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        if key and ANTHROPIC_AVAILABLE:
            try:
                self.client = anthropic.Anthropic(api_key=key)
                self.available = True
            except Exception:
                pass

    def ask(self, question, stream=False):
        if not self.available:
            return None

        # Enrich with real-time data
        enriched_question, _ = enrich_prompt_with_context(question)

        try:
            if stream:
                return self._ask_stream(enriched_question)
            else:
                response = self.client.messages.create(
                    model=self.model,
                    max_tokens=1000,
                    system=SYSTEM_PROMPT,
                    messages=[{"role": "user", "content": enriched_question}]
                )
                return response.content[0].text
        except Exception as e:
            return f"Error: {str(e)}"

    def _ask_stream(self, question):
        try:
            with self.client.messages.stream(
                model=self.model,
                max_tokens=1000,
                system=SYSTEM_PROMPT,
                messages=[{"role": "user", "content": question}]
            ) as stream:
                for text in stream.text_stream:
                    yield text
        except Exception as e:
            yield f"Error: {str(e)}"

    def is_available(self):
        return self.available


class OpenAILLM:
    """GPT-powered responses."""

    def __init__(self, api_key=None, model="gpt-4o-mini"):
        self.model = model
        self.client = None
        self.available = False

        key = api_key or os.environ.get("OPENAI_API_KEY")
        if key and OPENAI_AVAILABLE:
            try:
                self.client = OpenAI(api_key=key)
                self.available = True
            except Exception:
                pass

    def ask(self, question, stream=False):
        if not self.available:
            return None

        # Enrich with real-time data
        enriched_question, _ = enrich_prompt_with_context(question)

        try:
            if stream:
                return self._ask_stream(enriched_question)
            else:
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": SYSTEM_PROMPT},
                        {"role": "user", "content": enriched_question}
                    ],
                    temperature=0.7,
                    max_tokens=1000
                )
                return response.choices[0].message.content
        except Exception as e:
            return f"Error: {str(e)}"

    def _ask_stream(self, question):
        try:
            stream = self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": question}
                ],
                temperature=0.7,
                max_tokens=1000,
                stream=True
            )
            for chunk in stream:
                if chunk.choices[0].delta.content:
                    yield chunk.choices[0].delta.content
        except Exception as e:
            yield f"Error: {str(e)}"

    def is_available(self):
        return self.available


class JarvisLLM:
    """
    Multi-provider LLM wrapper.
    Priority: Ollama (free/local) > Anthropic > OpenAI
    JARVIS - Just A Rather Very Intelligent System
    """

    def __init__(self):
        # Try providers in order of preference
        self.ollama = OllamaLLM()
        self.anthropic = AnthropicLLM()
        self.openai = OpenAILLM()

        # Pick the best available (Ollama first since it's free)
        if self.ollama.is_available():
            self.provider = self.ollama
            self.provider_name = f"Ollama ({self.ollama.model})"
        elif self.anthropic.is_available():
            self.provider = self.anthropic
            self.provider_name = "Claude"
        elif self.openai.is_available():
            self.provider = self.openai
            self.provider_name = "GPT"
        else:
            self.provider = None
            self.provider_name = None

    def ask(self, question, stream=False):
        if not self.provider:
            return None
        return self.provider.ask(question, stream=stream)

    def is_available(self):
        return self.provider is not None


# Singleton
_llm_instance = None


def get_llm():
    """Get or create the LLM instance."""
    global _llm_instance
    if _llm_instance is None:
        _llm_instance = JarvisLLM()
    return _llm_instance


def quick_ask(question):
    """Quick one-liner to ask a question."""
    llm = get_llm()
    return llm.ask(question)
