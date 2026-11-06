#!/usr/bin/env python3
"""Gera os SVGs do ícone do LR Scanner: uma Discovery 4 (L319) preta de perfil.

Ilustração original, desenhada em METROS com as proporções reais do carro
(comprimento 4,83 m, entre-eixos 2,885 m, altura ~1,88 m, rodas de 19") e
convertida para pixels. Sem logotipos/emblemas de marca.

Uso:  python3 tool/icon_gen.py assets/brand
Saída (1024x1024):
  icon_full.svg     fundo de ponta a ponta + carro (ico, web maskable, base)
  icon_rounded.svg  igual, recortado em quadrado arredondado (web, logo)
  icon_macos.svg    no gabarito do macOS (quadrado arredondado 824 + sombra)
  icon_fg.svg       só o carro, na zona segura do ícone adaptativo (Android)
  icon_bg.svg       só o fundo (camada de fundo do ícone adaptativo)
  icon_mono.svg     silhueta branca (ícone temático do Android 13+)
Rasterize com tool/make_icons.sh.
"""
import math
import os
import sys

# ─── geometria do carro (metros; x a partir do para-choque dianteiro, y do chão) ───
FRONT_WHEEL = 0.87
WHEELBASE = 2.885
REAR_WHEEL = FRONT_WHEEL + WHEELBASE
TIRE_R = 0.38
RIM_R = 0.255
ARCH_R = 0.47
LENGTH = 4.84
HEIGHT = 1.93  # com o rack do teto


def arch_x(cx, y):
    """Pontos onde a caixa de roda (círculo) cruza a altura y."""
    dx = math.sqrt(ARCH_R ** 2 - (y - TIRE_R) ** 2)
    return cx - dx, cx + dx


class Car:
    """Desenha o carro com origem (para-choque, chão) em (x0, ground) e escala s px/m."""

    def __init__(self, x0, ground, s):
        self.x0, self.g, self.s = x0, ground, s

    def p(self, x, y):
        return f"{self.x0 + x * self.s:.1f},{self.g - y * self.s:.1f}"

    def X(self, x):
        return self.x0 + x * self.s

    def Y(self, y):
        return self.g - y * self.s

    def L(self, m):
        return m * self.s

    def path(self, pts, close=True):
        d = "M" + " L".join(self.p(x, y) for x, y in pts)
        return d + (" Z" if close else "")

    # Contorno da carroceria, com as caixas de roda recortadas.
    def body_d(self):
        p, r = self.p, self.L(ARCH_R)
        rf0, rf1 = arch_x(REAR_WHEEL, 0.50)
        ff0, ff1 = arch_x(FRONT_WHEEL, 0.47)
        return " ".join([
            f"M{p(0.10, 0.47)}",
            f"Q{p(0.0, 0.47)} {p(0.0, 0.58)}",      # canto inferior do para-choque
            f"L{p(0.0, 0.86)}",                     # face do para-choque
            f"Q{p(0.0, 1.04)} {p(0.12, 1.065)}",    # nariz do capô (frente reta, alta)
            f"L{p(0.85, 1.11)}",                    # capô (quase plano, alto)
            f"L{p(1.50, 1.155)}",                   # base do para-brisa
            f"L{p(2.03, 1.775)}",                   # para-brisa
            f"Q{p(2.08, 1.815)} {p(2.20, 1.815)}",  # topo do para-brisa
            f"L{p(2.70, 1.82)}",                    # teto dianteiro
            f"C{p(2.76, 1.82)} {p(2.78, 1.875)} {p(2.85, 1.875)}",  # o degrau do teto
            f"L{p(4.62, 1.875)}",                   # teto traseiro
            f"Q{p(4.78, 1.875)} {p(4.80, 1.76)}",   # canto do teto traseiro
            f"L{p(4.83, 1.02)}",                    # tampa traseira quase vertical
            f"L{p(4.84, 0.94)}",
            f"L{p(4.84, 0.60)}",
            f"Q{p(4.84, 0.50)} {p(4.74, 0.50)}",    # para-choque traseiro
            f"L{p(rf1, 0.50)}",
            f"A{r:.1f},{r:.1f} 0 0 0 {p(rf0, 0.50)}",  # caixa de roda traseira
            f"L{p(ff1, 0.47)}",                     # soleira
            f"A{r:.1f},{r:.1f} 0 0 0 {p(ff0, 0.47)}",  # caixa de roda dianteira
            "Z",
        ])

    def glass(self):
        """Vidros: dianteiro, traseiro, lateral traseira e as janelas alpinas."""
        p = self.p
        front = " ".join([
            f"M{p(1.64, 1.275)}", f"L{p(2.01, 1.72)}",
            f"Q{p(2.05, 1.755)} {p(2.13, 1.755)}",
            f"L{p(2.62, 1.76)}", f"L{p(2.62, 1.29)}", "Z",
        ])
        rear = self.path([(2.73, 1.30), (2.73, 1.775), (3.47, 1.80), (3.47, 1.315)])
        quarter = " ".join([
            f"M{p(3.58, 1.32)}", f"L{p(3.58, 1.80)}", f"L{p(4.42, 1.80)}",
            f"Q{p(4.54, 1.80)} {p(4.55, 1.68)}", f"L{p(4.56, 1.345)}", "Z",
        ])
        alpine = self.path([(2.90, 1.835), (3.50, 1.835), (3.50, 1.855), (2.90, 1.855)])
        return [front, rear, quarter], alpine

    def wheel(self, cx, uid):
        X, Y, L = self.X, self.Y, self.L
        x, y = X(cx), Y(TIRE_R)
        out = [
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(TIRE_R):.1f}" fill="#0b0d10"/>',
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(TIRE_R - 0.03):.1f}" fill="none" '
            f'stroke="#262c33" stroke-width="{L(0.012):.1f}"/>',
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(RIM_R):.1f}" fill="url(#rim)"/>',
        ]
        # 7 raios duplos: as aberturas escuras ficam entre eles.
        r0, r1 = L(0.075), L(RIM_R - 0.025)
        for i in range(7):
            a = math.radians(i * 360 / 7 + 90 / 7)
            w0, w1 = math.radians(9), math.radians(17)
            pts = [
                (x + r0 * math.cos(a - w0), y + r0 * math.sin(a - w0)),
                (x + r1 * math.cos(a - w1), y + r1 * math.sin(a - w1)),
                (x + r1 * math.cos(a + w1), y + r1 * math.sin(a + w1)),
                (x + r0 * math.cos(a + w0), y + r0 * math.sin(a + w0)),
            ]
            d = "M" + " L".join(f"{px:.1f},{py:.1f}" for px, py in pts) + " Z"
            out.append(f'<path d="{d}" fill="#14181d"/>')
        out += [
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(RIM_R):.1f}" fill="none" '
            f'stroke="#c9d0d6" stroke-width="{L(0.018):.1f}"/>',
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(0.06):.1f}" fill="#aab3bb"/>',
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{L(0.035):.1f}" fill="#2a3037"/>',
        ]
        return "\n".join(out)

    def svg(self, mono=False):
        X, Y, L, p = self.X, self.Y, self.L, self.p
        body = self.body_d()
        if mono:
            # Silhueta com os vidros e o vão das rodas vazados (lê melhor pequeno).
            windows, _ = self.glass()
            holes = "".join(f'<path d="{d}" fill="#000"/>' for d in windows)
            wheels = ""
            for c in (FRONT_WHEEL, REAR_WHEEL):
                cx, cy = X(c), Y(TIRE_R)
                holes += f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{L(ARCH_R - 0.035):.1f}" fill="#000"/>'
                wheels += (f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{L(TIRE_R - 0.02):.1f}" fill="#fff"/>'
                           f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{L(RIM_R - 0.03):.1f}" fill="#000"/>'
                           f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{L(0.08):.1f}" fill="#fff"/>')
            return (f'<mask id="m"><path d="{body}" fill="#fff"/>{holes}{wheels}</mask>'
                    f'<rect width="100%" height="100%" fill="#fff" mask="url(#m)"/>')

        windows, alpine = self.glass()
        out = []
        # sombra no chão
        out.append(
            f'<ellipse cx="{X(LENGTH / 2):.1f}" cy="{Y(0.0):.1f}" rx="{L(2.75):.1f}" '
            f'ry="{L(0.10):.1f}" fill="#000" opacity="0.55" filter="url(#soft)"/>')
        # forro das caixas de roda
        for c in (FRONT_WHEEL, REAR_WHEEL):
            out.append(f'<circle cx="{X(c):.1f}" cy="{Y(TIRE_R):.1f}" '
                       f'r="{L(ARCH_R - 0.01):.1f}" fill="#050607"/>')
        # rack do teto
        out.append(f'<path d="{self.path([(2.95, 1.875), (3.0, 1.915), (4.50, 1.915), (4.55, 1.875)])}" '
                   f'fill="#0d1014"/>')
        # carroceria + brilho do ombro + contorno de luz
        out.append(f'<path d="{body}" fill="url(#paint)"/>')
        out.append(f'<path d="{body}" fill="url(#sheen)"/>')
        out.append(f'<path d="{body}" fill="url(#floor)"/>')
        out.append(f'<path d="{body}" fill="none" stroke="url(#rimlight)" '
                   f'stroke-width="{L(0.022):.1f}" stroke-linejoin="round"/>')
        # vidros (preto com reflexo frio)
        for d in windows:
            out.append(f'<path d="{d}" fill="url(#glass)"/>')
            out.append(f'<path d="{d}" fill="url(#streak)"/>')
        out.append(f'<path d="{alpine}" fill="#3a4c58"/>')
        # linhas das portas
        line = f'stroke="#2c343d" stroke-width="{L(0.012):.1f}" fill="none" stroke-linecap="round"'
        out.append(f'<path d="M{p(1.60, 1.24)} L{p(1.60, 0.62)} Q{p(1.60, 0.53)} {p(1.68, 0.53)}" {line}/>')
        out.append(f'<path d="M{p(2.675, 1.80)} L{p(2.675, 0.53)}" {line}/>')
        out.append(f'<path d="M{p(3.525, 1.815)} L{p(3.525, 0.98)} Q{p(3.525, 0.88)} {p(3.40, 0.84)}" {line}/>')
        out.append(f'<path d="M{p(0.55, 0.74)} L{p(4.30, 0.76)}" stroke="#222930" '
                   f'stroke-width="{L(0.010):.1f}" fill="none" opacity="0.8"/>')
        # maçanetas
        for x0 in (2.38, 3.24):
            out.append(f'<rect x="{X(x0):.1f}" y="{Y(1.165):.1f}" width="{L(0.15):.1f}" '
                       f'height="{L(0.032):.1f}" rx="{L(0.016):.1f}" fill="#5d6873"/>')
        # retrovisor
        out.append(f'<path d="M{p(1.69, 1.27)} L{p(1.69, 1.40)} Q{p(1.69, 1.43)} {p(1.75, 1.43)} '
                   f'L{p(1.87, 1.42)} Q{p(1.90, 1.41)} {p(1.89, 1.36)} L{p(1.85, 1.27)} Z" fill="#101317"/>')
        out.append(f'<path d="M{p(1.71, 1.42)} L{p(1.87, 1.415)}" stroke="#56616c" '
                   f'stroke-width="{L(0.012):.1f}" stroke-linecap="round"/>')
        # "guelra" lateral no para-lama dianteiro
        gx, gy, gw, gh = X(1.30), Y(1.075), L(0.21), L(0.115)
        out.append(f'<rect x="{gx:.1f}" y="{gy:.1f}" width="{gw:.1f}" height="{gh:.1f}" '
                   f'rx="{L(0.02):.1f}" fill="#151a1f" stroke="#59646f" stroke-width="{L(0.01):.1f}"/>')
        for k in range(3):
            yy = gy + gh * (k + 1) / 4
            out.append(f'<path d="M{gx + L(0.03):.1f},{yy:.1f} H{gx + gw - L(0.03):.1f}" '
                       f'stroke="#7d8892" stroke-width="{L(0.012):.1f}" stroke-linecap="round"/>')
        # farol (DRL) e lanterna
        out.append(f'<path d="M{p(0.008, 0.875)} L{p(0.02, 0.99)} Q{p(0.05, 1.035)} {p(0.16, 1.04)} '
                   f'L{p(0.50, 1.06)} Q{p(0.52, 1.0)} {p(0.46, 0.97)} L{p(0.30, 0.91)} '
                   f'Q{p(0.16, 0.87)} {p(0.04, 0.87)} Z" fill="url(#lamp)"/>')
        out.append(f'<path d="M{p(0.05, 0.955)} L{p(0.42, 1.02)}" stroke="#ffffff" '
                   f'stroke-width="{L(0.018):.1f}" stroke-linecap="round" opacity="0.9"/>')
        out.append(f'<path d="M{p(0.03, 0.70)} L{p(0.14, 0.70)} L{p(0.14, 0.66)} L{p(0.02, 0.66)} Z" '
                   f'fill="#8f99a3" opacity="0.8"/>')
        out.append(f'<path d="M{p(4.765, 1.08)} L{p(4.785, 1.52)} L{p(4.81, 1.52)} L{p(4.835, 1.08)} Z" '
                   f'fill="#d3343a"/>')
        # rodas
        out.append(self.wheel(FRONT_WHEEL, "f"))
        out.append(self.wheel(REAR_WHEEL, "r"))
        return "\n".join(out)

    def defs(self):
        """Gradientes em coordenadas de usuário, presos à altura do carro."""
        Y = self.Y
        return f'''
  <linearGradient id="paint" gradientUnits="userSpaceOnUse" x1="0" y1="{Y(HEIGHT):.1f}" x2="0" y2="{Y(0.45):.1f}">
    <stop offset="0" stop-color="#454e58"/>
    <stop offset="0.30" stop-color="#20262d"/>
    <stop offset="0.56" stop-color="#14181d"/>
    <stop offset="0.62" stop-color="#2b333b"/>
    <stop offset="0.70" stop-color="#101317"/>
    <stop offset="1" stop-color="#07090b"/>
  </linearGradient>
  <linearGradient id="sheen" gradientUnits="userSpaceOnUse" x1="{self.X(0):.1f}" y1="0" x2="{self.X(LENGTH):.1f}" y2="0">
    <stop offset="0" stop-color="#ffffff" stop-opacity="0.00"/>
    <stop offset="0.35" stop-color="#bfeee8" stop-opacity="0.10"/>
    <stop offset="0.55" stop-color="#ffffff" stop-opacity="0.02"/>
    <stop offset="1" stop-color="#ffffff" stop-opacity="0.00"/>
  </linearGradient>
  <linearGradient id="rimlight" gradientUnits="userSpaceOnUse" x1="0" y1="{Y(HEIGHT):.1f}" x2="0" y2="{Y(0.7):.1f}">
    <stop offset="0" stop-color="#9ff7ec" stop-opacity="1"/>
    <stop offset="0.6" stop-color="#5fd9c9" stop-opacity="0.55"/>
    <stop offset="1" stop-color="#2dd4bf" stop-opacity="0.0"/>
  </linearGradient>
  <linearGradient id="floor" gradientUnits="userSpaceOnUse" x1="0" y1="{Y(0.42):.1f}" x2="0" y2="{Y(0.95):.1f}">
    <stop offset="0" stop-color="#2dd4bf" stop-opacity="0.16"/>
    <stop offset="1" stop-color="#2dd4bf" stop-opacity="0"/>
  </linearGradient>
  <linearGradient id="glass" gradientUnits="userSpaceOnUse" x1="0" y1="{Y(1.82):.1f}" x2="0" y2="{Y(1.27):.1f}">
    <stop offset="0" stop-color="#4b6170"/>
    <stop offset="0.45" stop-color="#1c2a34"/>
    <stop offset="1" stop-color="#0b1117"/>
  </linearGradient>
  <linearGradient id="streak" gradientUnits="userSpaceOnUse" x1="{self.X(1.9):.1f}" y1="{Y(1.85):.1f}" x2="{self.X(2.6):.1f}" y2="{Y(1.2):.1f}" spreadMethod="repeat">
    <stop offset="0" stop-color="#ffffff" stop-opacity="0"/>
    <stop offset="0.42" stop-color="#ffffff" stop-opacity="0"/>
    <stop offset="0.5" stop-color="#d9fffa" stop-opacity="0.16"/>
    <stop offset="0.58" stop-color="#ffffff" stop-opacity="0"/>
    <stop offset="1" stop-color="#ffffff" stop-opacity="0"/>
  </linearGradient>
  <radialGradient id="rim" cx="0.42" cy="0.38" r="0.75">
    <stop offset="0" stop-color="#f4f7f9"/>
    <stop offset="0.6" stop-color="#b9c2ca"/>
    <stop offset="1" stop-color="#7c8791"/>
  </radialGradient>
  <linearGradient id="lamp" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0" stop-color="#ffffff"/>
    <stop offset="1" stop-color="#b8e9ff"/>
  </linearGradient>
  <filter id="soft" x="-20%" y="-200%" width="140%" height="500%">
    <feGaussianBlur stdDeviation="{self.L(0.06):.1f}"/>
  </filter>'''


# ─── fundo e composições ───

def background(size, glow_cx, glow_cy, glow_r, line_y=None, line_w=None):
    """Grafite com brilho verde-água atrás do carro e a linha de 'scanner'."""
    out = [
        f'<rect width="{size}" height="{size}" fill="url(#bg)"/>',
        f'<circle cx="{glow_cx:.1f}" cy="{glow_cy:.1f}" r="{glow_r:.1f}" fill="url(#glow)"/>',
    ]
    if line_y is not None:
        out.append(scan_line(size / 2, line_y, line_w))
    return "\n".join(out)


def scan_line(cx, y, w):
    h = max(w * 0.012, 3)
    return (f'<rect x="{cx - w / 2:.1f}" y="{y - h * 2.2:.1f}" width="{w:.1f}" height="{h * 4.4:.1f}" '
            f'fill="url(#scanglow)" filter="url(#lineblur)"/>'
            f'<rect x="{cx - w / 2:.1f}" y="{y - h / 2:.1f}" width="{w:.1f}" height="{h:.1f}" '
            f'rx="{h / 2:.1f}" fill="url(#scan)"/>')


BG_DEFS = '''
  <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#1b2933"/>
    <stop offset="0.55" stop-color="#0f171d"/>
    <stop offset="1" stop-color="#070b0e"/>
  </linearGradient>
  <radialGradient id="glow">
    <stop offset="0" stop-color="#2dd4bf" stop-opacity="0.72"/>
    <stop offset="0.45" stop-color="#1f9e8f" stop-opacity="0.30"/>
    <stop offset="1" stop-color="#0e3b37" stop-opacity="0"/>
  </radialGradient>
  <linearGradient id="scan" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0" stop-color="#2dd4bf" stop-opacity="0"/>
    <stop offset="0.5" stop-color="#b9fff5" stop-opacity="1"/>
    <stop offset="1" stop-color="#2dd4bf" stop-opacity="0"/>
  </linearGradient>
  <linearGradient id="scanglow" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0" stop-color="#2dd4bf" stop-opacity="0"/>
    <stop offset="0.5" stop-color="#2dd4bf" stop-opacity="0.9"/>
    <stop offset="1" stop-color="#2dd4bf" stop-opacity="0"/>
  </linearGradient>
  <filter id="lineblur" x="-10%" y="-300%" width="120%" height="700%">
    <feGaussianBlur stdDeviation="6"/>
  </filter>'''


def doc(size, defs, body, clip=None):
    clip_def = clip_open = clip_close = ""
    if clip:
        clip_def = f'<clipPath id="shape">{clip}</clipPath>'
        clip_open, clip_close = '<g clip-path="url(#shape)">', "</g>"
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" '
            f'viewBox="0 0 {size} {size}">\n<defs>{defs}\n{clip_def}</defs>\n'
            f'{clip_open}\n{body}\n{clip_close}\n</svg>\n')


def composed(size, car_width, car_cy, rounded=None):
    """Fundo + brilho + linha + carro, com o carro de largura car_width centrado."""
    s = car_width / LENGTH
    ground = car_cy + HEIGHT * s / 2
    car = Car(size / 2 - car_width / 2, ground, s)
    bg = background(size, size / 2, car_cy - HEIGHT * s * 0.05, car_width * 0.62,
                    line_y=ground + s * 0.01, line_w=car_width * 1.18)
    clip = None
    if rounded:
        x, w, r = rounded
        clip = f'<rect x="{x}" y="{x}" width="{w}" height="{w}" rx="{r}"/>'
    return doc(size, BG_DEFS + car.defs(), bg + "\n" + car.svg(), clip)


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    S = 1024

    def write(name, text):
        with open(os.path.join(out_dir, name), "w") as f:
            f.write(text)

    # Ícone cheio (carro ocupa ~82% da largura).
    write("icon_full.svg", composed(S, S * 0.86, S * 0.50))
    write("icon_rounded.svg", composed(S, S * 0.86, S * 0.50, rounded=(0, S, S * 0.225)))

    # macOS (gabarito Big Sur+): quadrado arredondado 824 em 1024, com sombra.
    inner = 824
    off = (S - inner) / 2
    mac = composed(S, inner * 0.80, S * 0.50, rounded=(off, inner, 185))
    shadow = (f'<rect x="{off}" y="{off + 10}" width="{inner}" height="{inner}" rx="185" '
              f'fill="#000" opacity="0.35" filter="url(#macshadow)"/>')
    mac = mac.replace("<defs>", '<defs><filter id="macshadow" x="-20%" y="-20%" width="140%" '
                      'height="140%"><feGaussianBlur stdDeviation="14"/></filter>', 1)
    mac = mac.replace('<g clip-path="url(#shape)">', shadow + '\n<g clip-path="url(#shape)">', 1)
    write("icon_macos.svg", mac)

    # Adaptativo (Android 8+): canvas 108dp = 1024px; zona segura = círculo de 66dp.
    # Um retângulo 4,84 x 1,93 inscrito no círculo de 66dp: largura ~61dp.
    dp = S / 108
    car_w = 60 * dp
    s = car_w / LENGTH
    car_cy = S / 2 + 1.5 * dp
    ground = car_cy + HEIGHT * s / 2
    car = Car(S / 2 - car_w / 2, ground, s)
    fg_body = scan_line(S / 2, ground + s * 0.01, car_w * 1.12) + "\n" + car.svg()
    write("icon_fg.svg", doc(S, BG_DEFS + car.defs(), fg_body))
    write("icon_bg.svg", doc(S, BG_DEFS, background(S, S / 2, car_cy - HEIGHT * s * 0.05, car_w * 0.75)))
    write("icon_mono.svg", doc(S, "", car.svg(mono=True)))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "assets/brand")
