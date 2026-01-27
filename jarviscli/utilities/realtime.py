# -*- encoding: utf-8 -*-
"""
Real-time data fetching for Jarvis - Weather & News
"""
import requests
from datetime import datetime

WEATHER_API_KEY = "fde45bd61b21f91fbef3390c50880328"
NEWS_API_KEY = "1d0a89008a144bc98a07ec4b4f010ea5"


def get_weather(location="auto"):
    """
    Fetch current weather data.
    If location is "auto", tries to get location from IP.
    """
    try:
        # Auto-detect location if needed
        if location == "auto":
            ip_resp = requests.get("http://ip-api.com/json/", timeout=5)
            if ip_resp.status_code == 200:
                ip_data = ip_resp.json()
                location = ip_data.get("city", "New York")

        # Fetch weather
        url = f"https://api.openweathermap.org/data/2.5/weather"
        params = {
            "q": location,
            "appid": WEATHER_API_KEY,
            "units": "imperial"
        }
        resp = requests.get(url, params=params, timeout=10)

        if resp.status_code == 200:
            data = resp.json()
            weather = {
                "location": data.get("name", location),
                "temp": round(data["main"]["temp"]),
                "feels_like": round(data["main"]["feels_like"]),
                "humidity": data["main"]["humidity"],
                "description": data["weather"][0]["description"],
                "wind_speed": round(data["wind"]["speed"]),
                "high": round(data["main"]["temp_max"]),
                "low": round(data["main"]["temp_min"]),
            }
            return weather
    except Exception as e:
        return {"error": str(e)}

    return {"error": "Could not fetch weather"}


def get_news(query=None, category=None, count=5):
    """
    Fetch latest news headlines.
    Can filter by query (search term) or category.
    Categories: business, entertainment, general, health, science, sports, technology
    """
    try:
        if query:
            url = "https://newsapi.org/v2/everything"
            params = {
                "q": query,
                "sortBy": "publishedAt",
                "pageSize": count,
                "apiKey": NEWS_API_KEY
            }
        else:
            url = "https://newsapi.org/v2/top-headlines"
            params = {
                "country": "us",
                "pageSize": count,
                "apiKey": NEWS_API_KEY
            }
            if category:
                params["category"] = category

        resp = requests.get(url, params=params, timeout=10)

        if resp.status_code == 200:
            data = resp.json()
            articles = []
            for article in data.get("articles", [])[:count]:
                articles.append({
                    "title": article.get("title", ""),
                    "source": article.get("source", {}).get("name", ""),
                    "description": article.get("description", ""),
                    "url": article.get("url", "")
                })
            return {"articles": articles, "count": len(articles)}
    except Exception as e:
        return {"error": str(e)}

    return {"error": "Could not fetch news"}


def format_weather_context(weather):
    """Format weather data for LLM context."""
    if "error" in weather:
        return f"Weather data unavailable: {weather['error']}"

    return f"""CURRENT WEATHER DATA for {weather['location']}:
- Temperature: {weather['temp']}°F (feels like {weather['feels_like']}°F)
- Conditions: {weather['description']}
- High/Low: {weather['high']}°F / {weather['low']}°F
- Humidity: {weather['humidity']}%
- Wind: {weather['wind_speed']} mph"""


def format_news_context(news):
    """Format news data for LLM context."""
    if "error" in news:
        return f"News data unavailable: {news['error']}"

    lines = ["LATEST NEWS HEADLINES:"]
    for i, article in enumerate(news.get("articles", []), 1):
        lines.append(f"{i}. {article['title']} ({article['source']})")
        if article.get('description'):
            lines.append(f"   {article['description'][:150]}...")

    return "\n".join(lines)


def detect_query_type(query):
    """
    Detect if query is asking about weather or news.
    Returns: 'weather', 'news', or None
    """
    query_lower = query.lower()

    weather_keywords = ['weather', 'temperature', 'forecast', 'rain', 'snow', 'sunny',
                        'cloudy', 'humid', 'cold', 'hot', 'warm', 'outside', 'degrees']
    news_keywords = ['news', 'headlines', 'happening', 'latest', 'current events',
                     'whats going on', "what's going on", 'today', 'breaking']

    for kw in weather_keywords:
        if kw in query_lower:
            return 'weather'

    for kw in news_keywords:
        if kw in query_lower:
            return 'news'

    return None


def extract_location(query):
    """Try to extract a location from the query."""
    query_lower = query.lower()

    # Common patterns
    patterns = ['weather in ', 'weather for ', 'temperature in ', 'forecast for ']
    for pattern in patterns:
        if pattern in query_lower:
            idx = query_lower.find(pattern) + len(pattern)
            location = query[idx:].strip().rstrip('?').strip()
            if location:
                return location

    return "auto"
