# -*- coding: utf-8 -*-
import json
import requests
from . import mapps
from packages import weather_pinpoint as pinpoint
from packages.memory.memory import Memory
from colorama import Fore

# OpenWeatherMap API key
WEATHER_API_KEY = "fde45bd61b21f91fbef3390c50880328"


def main(self, s):
    # Trim input command to get only the location
    loc = s.replace(
        'weather',
        '').replace(
        'in ',
        '').replace(
            'at ',
        '').strip()

    # Try to get country, default to US for Fahrenheit
    try:
        location_data = mapps.get_location()
        country = location_data.get('country_name', location_data.get('country', 'United States'))
    except Exception:
        country = 'United States'

    # If country is US, shows weather in Fahrenheit
    if country == 'United States' or country == 'US':
        send_url = (
            f"http://api.openweathermap.org/data/2.5/weather?q={loc}"
            f"&APPID={WEATHER_API_KEY}&units=imperial"
        )
        unit = ' ºF in '

    # If country is not US, shows weather in Celsius
    else:
        send_url = (
            f"http://api.openweathermap.org/data/2.5/weather?q={loc}"
            f"&APPID={WEATHER_API_KEY}&units=metric"
        )
        unit = ' ºC in '

    try:
        r = requests.get(send_url, timeout=10)
        j = json.loads(r.text)
    except Exception as e:
        print(f"{Fore.RED}Could not fetch weather data: {e}{Fore.RESET}")
        return

    if 'message' in list(
            j.keys()) and (
            'city not found' in j['message'] or 'Nothing to geocode' in j['message']):
        print("Location invalid. Please be more specific")
        return pinpoint.main(Memory(), self, s)

    if 'main' not in j:
        print(f"{Fore.RED}Could not get weather for that location.{Fore.RESET}")
        return

    temperature = j['main']['temp']
    description = j['weather'][0]['main']
    location = j['name']
    print("{COLOR}It's {TEMP}{UNIT}{LOCATION} ({DESCRIPTION}){COLOR_RESET}"
          .format(COLOR=Fore.BLUE, COLOR_RESET=Fore.RESET,
                  TEMP=temperature, UNIT=unit, LOCATION=location.title(),
                  DESCRIPTION=description))
