#!/usr/bin/env python3
"""Genera los assets visuales de ALEXANDRIA (SVG) con código:
   - docs/assets/logo.svg          — sol verginiano + laurel + ΑΛΕΞΑΝΔΡΕΙΑ
   - docs/assets/architecture.svg  — Claude Code ↔ motor ↔ cadena, phalanx, LSP, hot reload
   - docs/assets/iterate-loop.svg  — ciclo R24 + flujo del agente autónomo
Ejecución: python3 scripts/gen-assets.py
Doc-min: los SVG NUNCA se editan a mano — este script es la única fuente.
"""
import os
from html import escape as esc

ROOT = os.path.join(os.path.dirname(__file__), "..", "docs", "assets")
os.makedirs(ROOT, exist_ok=True)

# ── Paleta ─────────────────────────────────────────────────────────────
DARK   = "#0b0f1a"   # fondo
PANEL  = "#141b2b"   # paneles
PANEL2 = "#1a2336"   # paneles elevados
EDGE   = "#26314b"   # bordes sutiles
ACCENT = "#e8b64c"   # oro macedonio
GOLD2  = "#f5d78e"   # oro claro (gradiente)
GREEN  = "#3ddc84"
BLUE   = "#4a9eff"
RED    = "#e0526e"
TEXT   = "#eceff7"
MUTED  = "#8a94ab"
FAINT  = "#3a465f"

SERIF = "Georgia, 'Times New Roman', serif"
SANS  = "'Segoe UI', system-ui, sans-serif"
MONO  = "'JetBrains Mono', 'Fira Code', monospace"

# ── Helpers ────────────────────────────────────────────────────────────
def panel(x, y, w, h, title=None, color=ACCENT, rx=14, sw=1.4):
    """Panel con borde superior acentuado + título."""
    out = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="url(#pgrad)" stroke="{EDGE}" stroke-width="1"/>']
    out.append(f'<rect x="{x}" y="{y}" width="{w}" height="3" rx="1.5" fill="{color}" opacity="0.9"/>')
    if title:
        out.append(f'<text x="{x+18}" y="{y+30}" font-family="{SANS}" font-size="14.5" font-weight="600" fill="{TEXT}">{esc(title)}</text>')
    return "\n  ".join(out)

def chip(x, y, w, label, sub=None, color=MUTED, h=26):
    """Chip técnico (monospace) con etiqueta y sub opcional."""
    out = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="{DARK}" stroke="{EDGE}" stroke-width="1"/>',
           f'<text x="{x+w/2}" y="{y+(16 if sub else 17.5)}" text-anchor="middle" font-family="{MONO}" font-size="10.5" fill="{TEXT}">{esc(label)}</text>']
    if sub:
        out.append(f'<text x="{x+w/2}" y="{y+23}" text-anchor="middle" font-family="{MONO}" font-size="8.5" fill="{color}">{esc(sub)}</text>')
    return "\n  ".join(out)

def arrow(x1, y1, x2, y2, color=ACCENT, label=None, dash=False, op=1.0, lw=1.8):
    d = f' stroke-dasharray="6 4"' if dash else ""
    lbl = f'\n  <text x="{(x1+x2)/2}" y="{(y1+y2)/2-8}" text-anchor="middle" font-family="{SANS}" font-size="9.5" fill="{MUTED}">{esc(label)}</text>' if label else ""
    return f'<path d="M{x1} {y1} L{x2} {y2}" stroke="{color}" stroke-width="{lw}"{d} opacity="{op}" marker-end="url(#arr)" fill="none"/>{lbl}'

DEFS = f"""<defs>
    <radialGradient id="bgGlow" cx="50%" cy="30%" r="85%">
      <stop offset="0%" stop-color="#182338"/>
      <stop offset="100%" stop-color="{DARK}"/>
    </radialGradient>
    <linearGradient id="pgrad" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="{PANEL2}"/>
      <stop offset="100%" stop-color="{PANEL}"/>
    </linearGradient>
    <linearGradient id="gold" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="{GOLD2}"/>
      <stop offset="100%" stop-color="{ACCENT}"/>
    </linearGradient>
    <filter id="glow" x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="5" result="b"/>
      <feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
    <pattern id="grid" width="32" height="32" patternUnits="userSpaceOnUse">
      <path d="M32 0 L0 0 0 32" fill="none" stroke="{EDGE}" stroke-width="0.4" opacity="0.35"/>
    </pattern>
    <marker id="arr" markerWidth="9" markerHeight="7" refX="8" refY="3.5" orient="auto">
      <path d="M0 0 L9 3.5 L0 7 Z" fill="{ACCENT}"/>
    </marker>
    <marker id="arrg" markerWidth="9" markerHeight="7" refX="8" refY="3.5" orient="auto">
      <path d="M0 0 L9 3.5 L0 7 Z" fill="{GREEN}"/>
    </marker>
  </defs>"""

# ── LOGO: sol verginiano + laurel ──────────────────────────────────────
import math
def vergina_sun(cx, cy, r_in, r_out, n=16, color="url(#gold)"):
    """Sol de Vergina: N puntas cónicas alternando largo/corto."""
    parts = []
    for i in range(n):
        a = math.radians(i * 360 / n - 90)
        r = r_out if i % 2 == 0 else r_out * 0.62
        tipx, tipy = cx + r * math.cos(a), cy + r * math.sin(a)
        # base perpendicular
        bx, by = cx + r_in * math.cos(a), cy + r_in * math.sin(a)
        half = math.radians(8)
        p1 = (cx + r_in * math.cos(a - half), cy + r_in * math.sin(a - half))
        p2 = (cx + r_in * math.cos(a + half), cy + r_in * math.sin(a + half))
        parts.append(f'<path d="M{p1[0]:.1f} {p1[1]:.1f} L{tipx:.1f} {tipy:.1f} L{p2[0]:.1f} {p2[1]:.1f} Z" fill="{color}"/>')
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r_in}" fill="{color}"/>')
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r_in*0.45}" fill="{DARK}"/>')
    return "\n    ".join(parts)

def laurel(cx, cy, side=-1, leaves=7):
    """Rama de laurel: tallo curvo + pares de hojas elípticas."""
    out = [f'<path d="M{cx} {cy+38} Q {cx+side*34:.0f} {cy+8:.0f} {cx+side*30:.0f} {cy-38:.0f}" stroke="{GREEN}" stroke-width="2.2" fill="none" stroke-linecap="round"/>']
    for i in range(leaves):
        t = i / (leaves - 1)
        lx = cx + side * (6 + 26 * t)
        ly = cy + 30 - 62 * t
        ang = -70 * side + (14 * t * side)
        rot = ang if side > 0 else ang
        for s in (-1, 1):
            a = math.radians(rot + s * 52)
            ex, ey = lx + 11 * math.cos(a), ly + 11 * math.sin(a)
            out.append(f'<ellipse cx="{(lx+ex)/2:.1f}" cy="{(ly+ey)/2:.1f}" rx="7.2" ry="3.1" fill="{GREEN}" opacity="0.92" transform="rotate({math.degrees(a):.0f} {(lx+ex)/2:.1f} {(ly+ey)/2:.1f})"/>')
    return "\n    ".join(out)

LOGO = f"""<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240">
  {DEFS}
  <circle cx="120" cy="120" r="116" fill="url(#bgGlow)"/>
  <circle cx="120" cy="120" r="116" fill="url(#grid)"/>
  <circle cx="120" cy="120" r="114" fill="none" stroke="url(#gold)" stroke-width="3"/>
  <circle cx="120" cy="120" r="106" fill="none" stroke="{ACCENT}" stroke-width="0.7" stroke-dasharray="2 6" opacity="0.6"/>
  <circle cx="120" cy="120" r="74" fill="{DARK}" opacity="0.55"/>
  <!-- sol verginiano -->
  <g filter="url(#glow)">
    {vergina_sun(120, 86, 10, 44)}
  </g>
  <!-- laureles -->
  {laurel(88, 152, side=-1)}
  {laurel(152, 152, side=1)}
  <text x="120" y="176" text-anchor="middle" font-family="{SERIF}" font-size="24" fill="{TEXT}" font-weight="bold" letter-spacing="1">ΑΛΕΞΑΝΔΡΕΙΑ</text>
  <text x="120" y="196" text-anchor="middle" font-family="{MONO}" font-size="8.5" fill="{ACCENT}" letter-spacing="3.5">AUTONOMOUS ENGINE</text>
</svg>
"""

# ── ARCHITECTURE: contenido REAL (ciclo 12) ────────────────────────────
CRATES = [
    ("alx-gate", "verifica + LSP"), ("alx-critic", "critica"),
    ("alx-task", "decompone"), ("alx-harness", "fases"),
    ("alx-governor", "coste + entropía"), ("alx-memory", "recalls"),
    ("alx-mcp", "protocolo"), ("alx-bench", "mide"),
    ("alx-night", "autónomo nocturno"), ("alx-evolve", "aprende"),
    ("alx-audit", "doctor"), ("alx-agents", "spawn"),
]
crate_chips = []
for i, (name, role) in enumerate(CRATES):
    col, row = i % 2, i // 2
    x = 430 + col * 168
    y = 128 + row * 34
    crate_chips.append(chip(x, y, 156, name, role, h=28))

arch_chips = "\n    ".join(crate_chips)

ARCH = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1240" height="700" viewBox="0 0 1240 700">
  {DEFS}
  <rect width="1240" height="700" fill="url(#bgGlow)"/>
  <rect width="1240" height="700" fill="url(#grid)"/>
  <text x="620" y="44" text-anchor="middle" font-family="{SERIF}" font-size="24" fill="{TEXT}" letter-spacing="2">ALEXANDRIA</text>
  <text x="620" y="64" text-anchor="middle" font-family="{MONO}" font-size="10" fill="{MUTED}" letter-spacing="4">MOTOR DE DESARROLLO IA AUTÓNOMO · 17 CRATES · 221 TESTS</text>

  <!-- ── Claude Code ── -->
  {panel(50, 96, 280, 250, "Claude Code", ACCENT)}
  <g>
    {chip(70, 140, 240, "alx hook <evento>", "dispatcher phalanx (misión·memoria·docmin)", ACCENT, 30)}
    {chip(70, 178, 240, "MCP alexandria", "11 tools REALES · lsp.check", GREEN, 30)}
    {chip(70, 216, 240, "hooks CC", "iterate · guards · activity", BLUE, 30)}
    {chip(70, 254, 240, "skills · plugins · themes", "sol/luna · atg wrapper", MUTED, 30)}
  </g>

  <!-- ── Engine ── -->
  {panel(410, 96, 350, 420, "Motor — phalanx/config.toml", BLUE)}
  {arch_chips}
  <text x="585" y="390" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{MUTED}">221 tests · clippy 0 · evoluciona solo</text>
  <g>
    {chip(430, 408, 310, "alx evolve — watcher de harnesses", "promueve/retira con uso real", ACCENT, 28)}
    {chip(430, 444, 310, "LSP real — rust-analyzer · pyright · tsserver", "handshake + diagnostics como evidencia", GREEN, 28)}
  </g>

  <!-- ── Cadena ── -->
  {panel(880, 96, 310, 250, "Cadena de modelos", GREEN)}
  <g>
    {chip(900, 140, 270, "headroom :8788", "compresión de contexto", TEXT, 30)}
    {chip(900, 178, 270, "routa-gateway :3460", "máscara [1m] + gobernador de entropía", TEXT, 30)}
    {chip(900, 216, 270, "routatic :3456", "PROVIDER · failover automático", TEXT, 30)}
    {chip(900, 254, 270, "omniroute :20128", "fallback multi-proveedor", MUTED, 30)}
  </g>
  <text x="1035" y="322" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{ACCENT}">modelo vivo desde config · cero hardcode</text>

  <!-- ── Flujo session loop ── -->
  {panel(880, 380, 310, 136, "Ciclo en la sesión", ACCENT)}
  <g font-family="{MONO}" font-size="10" fill="{TEXT}">
    <text x="900" y="428">1. harness-new  →  ⚡ hot reload</text>
    <text x="900" y="450">2. dispatcher reinyecta el listado vivo</text>
    <text x="900" y="472">3. VERIFICA → CRITICA → MEJORA</text>
    <text x="900" y="494" fill="{GREEN}">4. evolve promueve con evidencia</text>
  </g>

  <!-- ── Night infra ── -->
  {panel(50, 380, 280, 136, "Infra", RED)}
  <g>
    {chip(70, 424, 240, "alx-night.timer 02:00", "informe + backlog autónomo", TEXT, 28)}
    {chip(70, 460, 240, "ledger + telemetría", "coste real por llamada", MUTED, 28)}
  </g>

  <!-- flechas -->
  {arrow(330, 180, 406, 180, ACCENT, "hooks")}
  {arrow(406, 240, 330, 240, GREEN, "tools MCP", op=0.9)}
  {arrow(760, 180, 876, 180, GREEN, "LLM")}
  {arrow(560, 96, 560, 74, BLUE)}
  <text x="575" y="78" font-family="{SANS}" font-size="9" fill="{MUTED}">config-driven: cambiar TOML = cambiar el sistema</text>
  <path d="M330 430 C 380 430, 380 500, 410 500" stroke="{RED}" stroke-width="1.6" fill="none" stroke-dasharray="5 4" marker-end="url(#arr)" opacity="0.8"/>
  <text x="368" y="516" text-anchor="middle" font-family="{SANS}" font-size="9" fill="{RED}">night</text>
</svg>
"""

# ── ITERATE LOOP: ciclo circular R24 + flujo del agente ────────────────
def ring_segment(cx, cy, r, a0, a1, color, w=26, label=None, lcol=None):
    p0 = (cx + r * math.cos(math.radians(a0)), cy + r * math.sin(math.radians(a0)))
    p1 = (cx + r * math.cos(math.radians(a1)), cy + r * math.sin(math.radians(a1)))
    large = 1 if (a1 - a0) % 360 > 180 else 0
    out = f'<path d="M{p0[0]:.1f} {p0[1]:.1f} A{r} {r} 0 {large} 1 {p1[0]:.1f} {p1[1]:.1f}" stroke="{color}" stroke-width="{w}" fill="none" stroke-linecap="butt" opacity="0.92"/>'
    if label:
        am = math.radians((a0 + a1) / 2)
        tx, ty = cx + (r + w / 2 + 14) * math.cos(am), cy + (r + w / 2 + 14) * math.sin(am)
        out += f'\n  <text x="{tx:.0f}" y="{ty:.0f}" text-anchor="middle" font-family="{SANS}" font-size="12.5" font-weight="600" fill="{lcol or color}">{label}</text>'
    return out

LOOP = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1240" height="460" viewBox="0 0 1240 460">
  {DEFS}
  <rect width="1240" height="460" fill="url(#bgGlow)"/>
  <rect width="1240" height="460" fill="url(#grid)"/>
  <text x="300" y="48" text-anchor="middle" font-family="{SERIF}" font-size="21" fill="{TEXT}" letter-spacing="1">Ciclo R24</text>
  <text x="300" y="68" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{MUTED}" letter-spacing="3">VERIFICA → CRITICA → MEJORA · itera solo</text>

  <!-- anillo del ciclo -->
  {ring_segment(300, 250, 96, -60, 40, GREEN, 30, "VERIFICA", GREEN)}
  {ring_segment(300, 250, 96, 50, 150, BLUE, 30, "CRITICA", BLUE)}
  {ring_segment(300, 250, 96, 160, 260, RED, 30, "MEJORA", RED)}
  {ring_segment(300, 250, 96, 270, 290, ACCENT, 30, None)}
  <text x="300" y="256" text-anchor="middle" font-family="{SERIF}" font-size="17" fill="{TEXT}">unidad</text>
  <text x="300" y="276" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{MUTED}">iter+1 auto</text>
  <text x="300" y="404" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{ACCENT}">state.toml por sesión · auto-iterate en cada commit</text>

  <!-- flujo del agente autónomo (10 pasos) -->
  <text x="880" y="48" text-anchor="middle" font-family="{SERIF}" font-size="21" fill="{TEXT}" letter-spacing="1">Arranque autónomo</text>
  <text x="880" y="68" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{MUTED}" letter-spacing="3">atg --dangerously-skip-permissions</text>
  <g>
    {chip(700, 96, 360, "1·2 PLAN → TASK + MEMORIA", "alx task add · memory.recall", ACCENT, 28)}
    {chip(700, 132, 360, "3·4 AGENTE → SKILLS", "agent-dispatch sugiere · skills-fetch si falta", BLUE, 28)}
    {chip(700, 168, 360, "5·6 PLUGINS → SUBAGENTES", "Task en paralelo si es grande", GREEN, 28)}
    {chip(700, 204, 360, "7·8 HARNESS → MCP", "harness-new (⚡ hot reload) · phalanx.status", RED, 28)}
    {chip(700, 240, 360, "9·10 EVOLVE → VERIFICA-CRITICA-MEJORA", "harness permanente si hay evidencia · commit", ACCENT, 28)}
  </g>
  <path d="M700 110 C 660 110, 640 130, 610 160" stroke="{ACCENT}" stroke-width="1.6" fill="none" stroke-dasharray="5 4" marker-end="url(#arr)" opacity="0.7"/>
  <text x="640" y="120" text-anchor="middle" font-family="{SANS}" font-size="9" fill="{MUTED}">cada unidad cicla R24</text>
  <text x="880" y="300" text-anchor="middle" font-family="{MONO}" font-size="9.5" fill="{GREEN}">harnesses temporales por sesión/proyecto · se retiran solos al cumplir objetivo</text>

  <!-- banda inferior: guardas -->
  {panel(70, 330, 1100, 96, "Guardas que lo OBLIGAN (deterministas, fuera del LLM)", ACCENT)}
  <g font-family="{MONO}" font-size="10.5" fill="{TEXT}">
    <text x="95" y="376">system-usage-guard</text><text x="95" y="394" fill="{MUTED}" font-size="9">tasks + MCP obligatorios con iter&gt;0</text>
    <text x="370" y="376">research-guard</text><text x="370" y="394" fill="{MUTED}" font-size="9">no cierra research a medias</text>
    <text x="560" y="376">skill-guard</text><text x="560" y="394" fill="{MUTED}" font-size="9">los pasos de la skill se ejecutan</text>
    <text x="770" y="376">gate-verify</text><text x="770" y="394" fill="{MUTED}" font-size="9">build real o no pasa</text>
    <text x="950" y="376">LSP</text><text x="950" y="394" fill="{MUTED}" font-size="9">diagnostics como evidencia</text>
  </g>
</svg>
"""

for name, content in [("logo.svg", LOGO), ("architecture.svg", ARCH), ("iterate-loop.svg", LOOP)]:
    path = os.path.join(ROOT, name)
    with open(path, "w") as f:
        f.write(content)
    print(f"✓ {path} ({len(content)} bytes)")
