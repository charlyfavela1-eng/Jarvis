import math

from colorama import Fore
from plugin import plugin

"""
Converts a decimal number into a 32-bit IEEE 754 floating point binary representation.
"""

@plugin("floating point")
def floating_point(s):

    if s == "":
        s = jarvis.input("What's your decimal base ten number? ")

    try:
        val = float(s.strip())
    except ValueError:
        return "Invalid input"

    #Sign bit calculation
    sign = "0"
    if val < 0:
        sign = "1"
    
    val = abs(val)

    if val == 0:
        return "0" + "0"*8 + "0"*23

    #Exponent and Mantissa calculation
    fractional_part, integer_part = math.modf(val)

    whole = bin(integer_part)[2:]
    
    #Convert fraction to binary (up to 32 bits)
    frac_bin = ""
    while len(frac_bin) < 32:
        fractional_part *= 2
        bit = int(fractional_part)
        frac_bin += str(bit)
        fractional_part -= bit
        if fractional_part == 0: 
            break
    
    if integer_part >= 1:
        exponent = len(whole) - 1
        mantissa_source = whole[1:] + frac_bin
    else:
        first_one = frac_bin.find('1')
        exponent = -(first_one + 1)
        mantissa_source = frac_bin[first_one + 1:]

    exponent_bits = bin(exponent + 127)[2:].zfill(8)

    if exponent < -126:
        jarvis.say("Underflow: The number is too small to be represented in 32-bit IEEE 754 format.", Fore.RED)
    elif exponent > 127:
        jarvis.say("Overflow: The number is too large to be represented in 32-bit IEEE 754 format.", Fore.RED)
    else:
        mantissa = mantissa_source[:23].ljust(23, '0')

        result =  sign + exponent_bits + mantissa
        jarvis.say(result, Fore.BLUE)