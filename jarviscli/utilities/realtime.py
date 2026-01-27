# -*- encoding: utf-8 -*-
"""
Real-time data fetching for Jarvis - Weather, News & OSINT (Shodan)
"""
import os
import requests
from datetime import datetime

WEATHER_API_KEY = "fde45bd61b21f91fbef3390c50880328"
NEWS_API_KEY = "1d0a89008a144bc98a07ec4b4f010ea5"
SHODAN_API_KEY = os.environ.get("SHODAN_API_KEY", "uJSeUuGvN4aEhVwCJzDMEQimOD8jZLkJ")

# Try to import Shodan
try:
    import shodan
    SHODAN_AVAILABLE = True
except ImportError:
    SHODAN_AVAILABLE = False


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


# ============== SHODAN OSINT FUNCTIONS ==============

def shodan_lookup_ip(ip_address):
    """
    Look up an IP address on Shodan for OSINT.
    Returns open ports, services, vulnerabilities, geolocation.
    """
    if not SHODAN_AVAILABLE:
        return {"error": "Shodan not installed. Run: pip install shodan"}

    try:
        api = shodan.Shodan(SHODAN_API_KEY)
        result = api.host(ip_address)

        return {
            "ip": result.get("ip_str"),
            "org": result.get("org", "Unknown"),
            "isp": result.get("isp", "Unknown"),
            "country": result.get("country_name", "Unknown"),
            "city": result.get("city", "Unknown"),
            "ports": result.get("ports", []),
            "hostnames": result.get("hostnames", []),
            "vulns": result.get("vulns", []),
            "last_update": result.get("last_update", "Unknown"),
            "services": [
                {
                    "port": s.get("port"),
                    "protocol": s.get("transport", "tcp"),
                    "service": s.get("product", s.get("_shodan", {}).get("module", "unknown")),
                    "banner": s.get("data", "")[:200]
                }
                for s in result.get("data", [])[:10]
            ]
        }
    except shodan.APIError as e:
        return {"error": str(e)}
    except Exception as e:
        return {"error": str(e)}


def shodan_search(query, limit=5):
    """
    Search Shodan for hosts matching a query.
    Useful for finding exposed services, vulnerable systems.
    """
    if not SHODAN_AVAILABLE:
        return {"error": "Shodan not installed"}

    try:
        api = shodan.Shodan(SHODAN_API_KEY)
        results = api.search(query, limit=limit)

        hosts = []
        for match in results.get("matches", [])[:limit]:
            hosts.append({
                "ip": match.get("ip_str"),
                "port": match.get("port"),
                "org": match.get("org", "Unknown"),
                "country": match.get("location", {}).get("country_name", "Unknown"),
                "product": match.get("product", "Unknown"),
                "banner": match.get("data", "")[:150]
            })

        return {
            "total": results.get("total", 0),
            "hosts": hosts
        }
    except shodan.APIError as e:
        return {"error": str(e)}
    except Exception as e:
        return {"error": str(e)}


def shodan_lookup_domain(domain):
    """
    Look up a domain on Shodan - get subdomains and IPs.
    """
    if not SHODAN_AVAILABLE:
        return {"error": "Shodan not installed"}

    try:
        api = shodan.Shodan(SHODAN_API_KEY)
        result = api.dns.domain_info(domain)

        return {
            "domain": domain,
            "subdomains": result.get("subdomains", [])[:20],
            "records": result.get("data", [])[:10]
        }
    except shodan.APIError as e:
        return {"error": str(e)}
    except Exception as e:
        return {"error": str(e)}


def format_shodan_context(data):
    """Format Shodan data for LLM context."""
    if "error" in data:
        return f"Shodan lookup error: {data['error']}"

    if "ip" in data:
        # IP lookup result
        lines = [f"SHODAN OSINT DATA for {data['ip']}:"]
        lines.append(f"- Organization: {data.get('org', 'Unknown')}")
        lines.append(f"- ISP: {data.get('isp', 'Unknown')}")
        lines.append(f"- Location: {data.get('city', 'Unknown')}, {data.get('country', 'Unknown')}")
        lines.append(f"- Open Ports: {', '.join(map(str, data.get('ports', [])))}")
        lines.append(f"- Hostnames: {', '.join(data.get('hostnames', [])) or 'None'}")

        if data.get('vulns'):
            lines.append(f"- VULNERABILITIES: {', '.join(data.get('vulns', []))}")

        if data.get('services'):
            lines.append("- Services:")
            for svc in data['services'][:5]:
                lines.append(f"  * Port {svc['port']}/{svc['protocol']}: {svc['service']}")

        return "\n".join(lines)

    elif "hosts" in data:
        # Search result
        lines = [f"SHODAN SEARCH RESULTS ({data.get('total', 0)} total):"]
        for h in data.get('hosts', []):
            lines.append(f"- {h['ip']}:{h['port']} ({h['country']}) - {h['product']}")
        return "\n".join(lines)

    elif "subdomains" in data:
        # Domain lookup
        lines = [f"SHODAN DOMAIN DATA for {data.get('domain', 'unknown')}:"]
        lines.append(f"- Subdomains: {', '.join(data.get('subdomains', [])[:10])}")
        return "\n".join(lines)

    return "No Shodan data available"


def detect_osint_query(query):
    """Detect if query is asking for OSINT/Shodan lookup."""
    query_lower = query.lower()

    osint_keywords = ['shodan', 'lookup ip', 'scan ip', 'osint', 'what ports',
                      'open ports', 'vulnerabilities on', 'recon', 'investigate ip',
                      'check ip', 'who owns ip', 'ip address']

    for kw in osint_keywords:
        if kw in query_lower:
            return True
    return False


def extract_ip_from_query(query):
    """Extract IP address from a query string."""
    import re
    ip_pattern = r'\b(?:\d{1,3}\.){3}\d{1,3}\b'
    match = re.search(ip_pattern, query)
    return match.group(0) if match else None


def extract_domain_from_query(query):
    """Extract domain from a query string."""
    import re
    domain_pattern = r'\b(?:[a-zA-Z0-9-]+\.)+[a-zA-Z]{2,}\b'
    matches = re.findall(domain_pattern, query)
    # Filter out common non-domains
    for m in matches:
        if not m.endswith(('.com', '.org', '.net', '.io', '.co', '.edu', '.gov')):
            continue
        return m
    return matches[0] if matches else None
