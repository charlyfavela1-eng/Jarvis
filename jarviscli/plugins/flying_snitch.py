"""
Flying Snitch Mechanism - Real-Life Golden Snitch Simulation
============================================================

Mecanismo de snitch voladora inspirado en el Quidditch de Harry Potter.

Modela el comportamiento de una snitch real usando:
  - Fisica de vuelo (posicion, velocidad, aceleracion en 3D)
  - Algoritmo de evasion reactiva basado en la posicion del jugador
  - Patron de vuelo erratico con perlin-like noise
  - Zona de captura con ventana de oportunidad corta

Diseño de hardware real sugerido:
  - Microcontrolador: Raspberry Pi Zero 2W o ESP32-S3
  - Propulsion: 4x motor coreless 7mm + helices de 55mm (frame ~65mm diagonal)
  - IMU: MPU-6050 para estabilizacion
  - Sensor de proximidad: ToF VL53L1X (deteccion de mano a <30cm)
  - Comunicacion: Bluetooth LE para telemetria y comandos
  - Alimentacion: LiPo 1S 300mAh (~4min de vuelo)
  - Carcasa: esfera dorada de 3.5cm impresa en PLA + pintura dorada
"""

import math
import random
import time
import threading

from colorama import Fore, Style
from plugin import plugin, alias


# ---------------------------------------------------------------------------
# Motor de fisica
# ---------------------------------------------------------------------------

class Vec3:
    """Vector 3D minimalista para la simulacion de vuelo."""

    def __init__(self, x=0.0, y=0.0, z=0.0):
        self.x = float(x)
        self.y = float(y)
        self.z = float(z)

    def __add__(self, other):
        return Vec3(self.x + other.x, self.y + other.y, self.z + other.z)

    def __sub__(self, other):
        return Vec3(self.x - other.x, self.y - other.y, self.z - other.z)

    def __mul__(self, scalar):
        return Vec3(self.x * scalar, self.y * scalar, self.z * scalar)

    def magnitude(self):
        return math.sqrt(self.x ** 2 + self.y ** 2 + self.z ** 2)

    def normalized(self):
        m = self.magnitude()
        if m < 1e-9:
            return Vec3()
        return Vec3(self.x / m, self.y / m, self.z / m)

    def clamp(self, max_mag):
        m = self.magnitude()
        if m > max_mag:
            return self.normalized() * max_mag
        return Vec3(self.x, self.y, self.z)

    def __repr__(self):
        return f"({self.x:.2f}, {self.y:.2f}, {self.z:.2f})"


class SnitchPhysics:
    """
    Modelo fisico de la snitch voladora.

    Sistema de coordenadas:
        X: este (+) / oeste (-)
        Y: altura (+arriba)
        Z: sur (+) / norte (-)

    Limites del campo de juego: cubo de FIELD_SIZE metros centrado en origen.
    """

    FIELD_SIZE = 10.0        # metros - radio del espacio de juego
    HOVER_HEIGHT = 1.5       # altura de hover preferida (m)
    MAX_SPEED = 6.0          # m/s velocidad maxima
    MAX_ACCEL = 8.0          # m/s^2 aceleracion maxima
    EVASION_RADIUS = 2.0     # metros - distancia en que activa evasion
    CATCH_RADIUS = 0.3       # metros - radio de captura exitosa
    NEAR_MISS_RADIUS = 0.8   # metros - radio para retroalimentacion tactil
    DRAG = 0.85              # coeficiente de arrastre por frame
    DT = 0.05                # delta de tiempo por tick (20 Hz)
    FLAP_PROB = 0.15         # probabilidad de aleteo (cambio brusco de dir)
    HOVER_BIAS = 0.3         # fuerza que atrae al hover height

    def __init__(self):
        self.pos = Vec3(
            random.uniform(-2, 2),
            random.uniform(1.2, 2.5),
            random.uniform(-2, 2),
        )
        self.vel = Vec3(
            random.uniform(-1, 1),
            random.uniform(-0.3, 0.3),
            random.uniform(-1, 1),
        )
        self.accel = Vec3()
        self._noise_target = self._random_target()
        self._noise_timer = 0.0
        self._evasion_active = False
        self.ticks = 0
        self.total_distance = 0.0

    # ------------------------------------------------------------------
    # Nucleo de simulacion
    # ------------------------------------------------------------------

    def _random_target(self):
        return Vec3(
            random.uniform(-self.FIELD_SIZE * 0.7, self.FIELD_SIZE * 0.7),
            random.uniform(0.8, 3.5),
            random.uniform(-self.FIELD_SIZE * 0.7, self.FIELD_SIZE * 0.7),
        )

    def _wander_force(self):
        """Fuerza de deambulacion hacia objetivo aleatorio cambiante."""
        self._noise_timer += self.DT
        if self._noise_timer > random.uniform(1.5, 4.0):
            self._noise_target = self._random_target()
            self._noise_timer = 0.0
        to_target = self._noise_target - self.pos
        return to_target.normalized() * random.uniform(1.0, 3.0)

    def _hover_force(self):
        """Fuerza de restauracion de altura (simula estabilizacion del IMU)."""
        delta_y = self.HOVER_HEIGHT - self.pos.y
        return Vec3(0, delta_y * self.HOVER_BIAS * 10, 0)

    def _evasion_force(self, player_pos: Vec3):
        """
        Evasion reactiva: huye de la posicion del jugador con intensidad
        proporcional a la proximidad al cubo de la inversa de la distancia.
        """
        to_player = player_pos - self.pos
        dist = to_player.magnitude()
        if dist > self.EVASION_RADIUS or dist < 1e-6:
            self._evasion_active = False
            return Vec3()
        self._evasion_active = True
        flee_dir = (self.pos - player_pos).normalized()
        intensity = (self.EVASION_RADIUS / max(dist, 0.1)) ** 2
        return flee_dir * intensity * self.MAX_ACCEL

    def _boundary_force(self):
        """Repulsion de limites del campo - evita que salga del espacio."""
        f = Vec3()
        for attr in ('x', 'z'):
            v = getattr(self.pos, attr)
            if v > self.FIELD_SIZE:
                setattr(f, attr, -(v - self.FIELD_SIZE) * 5)
            elif v < -self.FIELD_SIZE:
                setattr(f, attr, (-self.FIELD_SIZE - v) * 5)
        if self.pos.y < 0.3:
            f.y += (0.3 - self.pos.y) * 10
        elif self.pos.y > 5.0:
            f.y -= (self.pos.y - 5.0) * 5
        return f

    def _flap(self):
        """Aleteo esporadico: impulso en direccion aleatoria (realismo)."""
        if random.random() < self.FLAP_PROB:
            return Vec3(
                random.uniform(-4, 4),
                random.uniform(-1, 2),
                random.uniform(-4, 4),
            )
        return Vec3()

    def tick(self, player_pos: Vec3 | None = None):
        """Avanza la simulacion un paso de tiempo DT."""
        if player_pos is None:
            player_pos = Vec3(0, 0, 0)

        self.accel = (
            self._wander_force()
            + self._hover_force()
            + self._evasion_force(player_pos)
            + self._boundary_force()
            + self._flap()
        ).clamp(self.MAX_ACCEL)

        prev = Vec3(self.pos.x, self.pos.y, self.pos.z)
        self.vel = (self.vel + self.accel * self.DT).clamp(self.MAX_SPEED)
        self.vel = self.vel * self.DRAG
        self.pos = self.pos + self.vel * self.DT
        self.total_distance += (self.pos - prev).magnitude()
        self.ticks += 1

    # ------------------------------------------------------------------
    # Consultas de estado
    # ------------------------------------------------------------------

    def distance_to(self, other: Vec3) -> float:
        return (self.pos - other).magnitude()

    def is_caught_by(self, hand_pos: Vec3) -> bool:
        return self.distance_to(hand_pos) <= self.CATCH_RADIUS

    def near_miss(self, hand_pos: Vec3) -> bool:
        d = self.distance_to(hand_pos)
        return self.CATCH_RADIUS < d <= self.NEAR_MISS_RADIUS

    def speed(self) -> float:
        return self.vel.magnitude()

    def status_line(self) -> str:
        evasion = Fore.RED + "EVADIENDO" + Style.RESET_ALL if self._evasion_active else Fore.GREEN + "libre" + Style.RESET_ALL
        return (
            f"  Pos {self.pos}  |  "
            f"Vel {self.speed():.2f} m/s  |  "
            f"Dist recorrida {self.total_distance:.1f}m  |  "
            f"Estado: {evasion}"
        )


# ---------------------------------------------------------------------------
# Motor de captura (juego de cazador)
# ---------------------------------------------------------------------------

FIELD_WIDTH = 40
FIELD_HEIGHT = 12

_SNITCH_CHAR = Fore.YELLOW + Style.BRIGHT + "●" + Style.RESET_ALL
_PLAYER_CHAR = Fore.CYAN + Style.BRIGHT + "+" + Style.RESET_ALL


def _project_2d(pos: Vec3, field_w: int, field_h: int, field_size: float):
    """Proyeccion isometrica simplificada a cuadricula ASCII."""
    # Vista de planta XZ con altura codificada en brillo
    norm_x = (pos.x + field_size) / (2 * field_size)
    norm_z = (pos.z + field_size) / (2 * field_size)
    col = int(norm_x * (field_w - 1))
    row = int(norm_z * (field_h - 1))
    col = max(0, min(field_w - 1, col))
    row = max(0, min(field_h - 1, row))
    return col, row


def _render_field(snitch: SnitchPhysics, player: Vec3, turn: int):
    """Dibuja el campo de juego en ASCII."""
    grid = [["." for _ in range(FIELD_WIDTH)] for _ in range(FIELD_HEIGHT)]

    sc, sr = _project_2d(snitch.pos, FIELD_WIDTH, FIELD_HEIGHT, snitch.FIELD_SIZE)
    pc, pr = _project_2d(player, FIELD_WIDTH, FIELD_HEIGHT, snitch.FIELD_SIZE)

    # Marcador de jugador
    grid[pr][pc] = "P"
    # Marcador de snitch (sobreescribe si coincide -> captura detectada antes)
    grid[sr][sc] = "S"

    border = "+" + "-" * FIELD_WIDTH + "+"
    lines = [border]
    for r, row in enumerate(grid):
        rendered_row = ""
        for c, cell in enumerate(row):
            if cell == "S":
                rendered_row += _SNITCH_CHAR
            elif cell == "P":
                rendered_row += _PLAYER_CHAR
            else:
                rendered_row += Fore.WHITE + Style.DIM + "." + Style.RESET_ALL
        lines.append("|" + rendered_row + "|")
    lines.append(border)
    return "\n".join(lines)


def _parse_coords(raw: str, current: Vec3) -> Vec3 | None:
    """
    Interpreta comandos de movimiento del jugador.
    Formatos soportados:
        w / a / s / d      -> mover N/O/S/E 1m
        up / down          -> subir/bajar 0.5m
        x,y,z              -> posicion absoluta
        (enter)            -> quedarse quieto
    """
    raw = raw.strip().lower()
    if raw == "":
        return Vec3(current.x, current.y, current.z)
    if raw == "w":
        return Vec3(current.x, current.y, current.z - 1)
    if raw == "s":
        return Vec3(current.x, current.y, current.z + 1)
    if raw == "a":
        return Vec3(current.x - 1, current.y, current.z)
    if raw == "d":
        return Vec3(current.x + 1, current.y, current.z)
    if raw == "up":
        return Vec3(current.x, current.y + 0.5, current.z)
    if raw == "down":
        return Vec3(current.x, current.y - 0.5, current.z)
    parts = raw.replace(",", " ").split()
    if len(parts) == 3:
        try:
            return Vec3(float(parts[0]), float(parts[1]), float(parts[2]))
        except ValueError:
            pass
    return None


# ---------------------------------------------------------------------------
# Plugin principal
# ---------------------------------------------------------------------------

@alias("snitch")
@plugin("flying snitch")
def flying_snitch(jarvis, s):
    """
    Simulacion interactiva de una snitch voladora (Golden Snitch).

    Modela la fisica de vuelo, evasion y captura de una snitch real.
    Incluye motor de vuelo 3D con evasion reactiva y representacion ASCII.

    Comandos durante el juego:
        w/a/s/d   -> mover Norte/Oeste/Sur/Este
        up/down   -> subir/bajar
        x,y,z     -> teletransportar a coordenadas
        Enter     -> permanecer en posicion
        q         -> salir

    -- Example:
        flying snitch
        snitch
    """
    jarvis.say("")
    jarvis.say(
        Fore.YELLOW + Style.BRIGHT +
        "  ╔══════════════════════════════════════════════╗\n"
        "  ║   SNITCH VOLADORA  -  Mecanismo Real        ║\n"
        "  ╚══════════════════════════════════════════════╝"
        + Style.RESET_ALL
    )
    jarvis.say(
        Fore.CYAN +
        "\n  Campo de juego: 20x20x6 metros"
        "\n  S = Snitch  |  P = Jugador (tu)\n"
        + Style.RESET_ALL
    )
    jarvis.say("  [w/a/s/d = mover, up/down = altura, q = salir]\n")

    snitch = SnitchPhysics()
    player = Vec3(0, 1.5, 0)
    turns = 0
    caught = False
    near_misses = 0
    start_time = time.time()

    while True:
        # Avanzar simulacion varios ticks entre inputs del usuario
        for _ in range(4):
            snitch.tick(player)

        turns += 1
        jarvis.say(Fore.WHITE + Style.DIM + "\033[2J\033[H" + Style.RESET_ALL, end="")

        # Dibujar campo
        jarvis.say(_render_field(snitch, player, turns))
        jarvis.say(snitch.status_line())

        dist = snitch.distance_to(player)
        dist_color = (
            Fore.RED if dist < 1.5 else
            Fore.YELLOW if dist < 3.5 else
            Fore.GREEN
        )
        jarvis.say(
            f"\n  Turno {turns:>4}  |  "
            f"Distancia a snitch: {dist_color}{dist:.2f}m{Style.RESET_ALL}  |  "
            f"Casi-capturas: {near_misses}"
        )
        jarvis.say(
            f"  Tu posicion: ({player.x:.1f}, {player.y:.1f}, {player.z:.1f})  |  "
            f"Altura snitch: {Fore.YELLOW}{snitch.pos.y:.2f}m{Style.RESET_ALL}"
        )

        if snitch.near_miss(player):
            near_misses += 1
            jarvis.say(Fore.YELLOW + Style.BRIGHT + "\n  *** CASI LA TIENES! ***" + Style.RESET_ALL)

        if snitch.is_caught_by(player):
            caught = True
            break

        jarvis.say("\n  Movimiento > ", end="")
        cmd = jarvis.input()

        if cmd.strip().lower() == "q":
            break

        new_pos = _parse_coords(cmd, player)
        if new_pos is not None:
            player = new_pos
        else:
            jarvis.say(Fore.RED + "  Comando no reconocido. Usa w/a/s/d, up, down o x,y,z" + Style.RESET_ALL)

    elapsed = time.time() - start_time
    jarvis.say("\n")

    if caught:
        jarvis.say(
            Fore.YELLOW + Style.BRIGHT +
            "  ╔══════════════════════════════════════╗\n"
            "  ║   ¡¡ SNITCH CAPTURADA !!  ¡GANASTE! ║\n"
            "  ╚══════════════════════════════════════╝"
            + Style.RESET_ALL
        )
    else:
        jarvis.say(Fore.CYAN + "  La snitch sigue volando libre..." + Style.RESET_ALL)

    jarvis.say(
        f"\n  Estadisticas:"
        f"\n    Turnos jugados   : {turns}"
        f"\n    Tiempo de juego  : {elapsed:.1f}s"
        f"\n    Distancia snitch : {snitch.total_distance:.1f}m"
        f"\n    Casi-capturas    : {near_misses}"
        f"\n    Velocidad maxima : {snitch.MAX_SPEED} m/s"
        f"\n"
    )


# ---------------------------------------------------------------------------
# Subcomando: mostrar especificaciones de hardware real
# ---------------------------------------------------------------------------

@plugin("flying snitch specs")
def flying_snitch_specs(jarvis, s):
    """
    Muestra las especificaciones de hardware para construir
    una snitch voladora real basada en este mecanismo.

    -- Example:
        flying snitch specs
    """
    specs = f"""
{Fore.YELLOW + Style.BRIGHT}  ESPECIFICACIONES DE HARDWARE - SNITCH VOLADORA REAL{Style.RESET_ALL}
{Fore.CYAN}  ══════════════════════════════════════════════════{Style.RESET_ALL}

{Fore.WHITE + Style.BRIGHT}  ESTRUCTURA{Style.RESET_ALL}
    Diametro exterior  : 35 mm (esfera dorada)
    Peso total objetivo: < 28 g (limite de registro FAA/EASA clase <250g)
    Material carcasa   : PLA / resina + pintura metalica dorada
    Alas               : 2x alas articuladas (servo SG90, envergadura 12cm)

{Fore.WHITE + Style.BRIGHT}  PROPULSION{Style.RESET_ALL}
    Motores            : 4x coreless 7mm x 20mm (≈55000 RPM @3.7V)
    Helices            : 55mm bipaletas
    Configuracion      : cuadrotor X en frame 65mm diagonal
    Empuje total       : ~120g (relacion empuje/peso 4.3:1)

{Fore.WHITE + Style.BRIGHT}  ELECTRONICA{Style.RESET_ALL}
    Controlador vuelo  : ESP32-S3 + firmware ArduPilot/Betaflight
    IMU                : MPU-6050 (acelerometro + giroscopio 6-DOF)
    Sensor proximidad  : VL53L1X ToF (rango 4m, I2C 400kHz)
    Detector captura   : 4x FSR 402 en carcasa (presion de mano)
    Comunicacion       : Bluetooth LE 5.0 (telemetria + comandos)
    LED               : WS2812B x8 (efecto dorado parpadeante)

{Fore.WHITE + Style.BRIGHT}  ENERGIA{Style.RESET_ALL}
    Bateria            : LiPo 1S 300mAh 30C (3.7V nominal)
    Consumo hover      : ~18W → ≈60s autonomia en hover puro
    Consumo evasion    : ~28W → ≈40s en vuelo agresivo
    Carga              : USB-C 5V/1A via JST en la base

{Fore.WHITE + Style.BRIGHT}  ALGORITMO DE VUELO (este modulo){Style.RESET_ALL}
    Frecuencia control : 20 Hz (DT=50ms)
    Evasion            : activa a <{SnitchPhysics.EVASION_RADIUS}m del sensor ToF
    Captura exitosa    : FSR detecta presion >200g durante >150ms
    Radio de captura   : {SnitchPhysics.CATCH_RADIUS}m
    Velocidad maxima   : {SnitchPhysics.MAX_SPEED} m/s
    Aceleracion max    : {SnitchPhysics.MAX_ACCEL} m/s²

{Fore.WHITE + Style.BRIGHT}  FLUJO DE EVASION REAL{Style.RESET_ALL}
    1. VL53L1X detecta objeto a <{SnitchPhysics.EVASION_RADIUS}m
    2. ESP32-S3 calcula vector de fuga (algoritmo en este plugin)
    3. Mezcla vector de fuga con wander para imprevisibilidad
    4. PID de altitud mantiene hover ({SnitchPhysics.HOVER_HEIGHT}m ± 0.2m)
    5. Si FSR detecta captura → LEDs verdes + audio via buzzer

{Fore.CYAN}  Costo estimado de componentes: ~$45 USD{Style.RESET_ALL}
"""
    jarvis.say(specs)


# ---------------------------------------------------------------------------
# Subcomando: simulacion de telemetria en tiempo real (sin interaccion)
# ---------------------------------------------------------------------------

@plugin("flying snitch telemetry")
def flying_snitch_telemetry(jarvis, s):
    """
    Ejecuta la simulacion de vuelo autonomo y muestra telemetria
    en tiempo real durante 30 segundos (sin jugador).

    -- Example:
        flying snitch telemetry
    """
    jarvis.say(
        Fore.YELLOW + Style.BRIGHT +
        "\n  [TELEMETRIA SNITCH] Vuelo autonomo 30s - Ctrl+C para detener\n" +
        Style.RESET_ALL
    )
    jarvis.say(
        f"  {'Tick':>5}  {'X':>7}  {'Y':>7}  {'Z':>7}  "
        f"{'Vel(m/s)':>9}  {'Dist(m)':>8}  {'Estado':>12}"
    )
    jarvis.say("  " + "-" * 68)

    snitch = SnitchPhysics()
    try:
        for i in range(600):  # 30s a 20Hz
            snitch.tick()
            if i % 20 == 0:  # imprimir cada segundo
                state = "EVADIENDO" if snitch._evasion_active else "libre    "
                jarvis.say(
                    f"  {snitch.ticks:>5}  "
                    f"{snitch.pos.x:>7.2f}  "
                    f"{snitch.pos.y:>7.2f}  "
                    f"{snitch.pos.z:>7.2f}  "
                    f"{snitch.speed():>9.2f}  "
                    f"{snitch.total_distance:>8.2f}  "
                    f"  {state}"
                )
                time.sleep(snitch.DT * 20)
    except KeyboardInterrupt:
        pass

    jarvis.say(
        f"\n  Simulacion completada."
        f"\n  Distancia total recorrida : {snitch.total_distance:.2f}m"
        f"\n  Ticks procesados          : {snitch.ticks}"
        f"\n  Posicion final            : {snitch.pos}\n"
    )
