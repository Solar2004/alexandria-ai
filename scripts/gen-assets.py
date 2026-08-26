#!/usr/bin/env python3
"""Genera los assets visuales de ALEXANDRIA (SVG) con código:
   - docs/assets/logo.svg          — logo circular (Lábaro con ΑΛΕΞΑΝΔΡΕΙΑ)
   - docs/assets/architecture.svg  — diagrama de arquitectura (chain + engine + claude)
   - docs/assets/iterate-loop.svg  — bucle de iteración R24
Ejecución: python3 scripts/gen-assets.py
"""
import os

ROOT = os.path.join(os.path.dirname(__file__), "..", "docs", "assets")
os.makedirs(ROOT, exist_ok=True)

# ── Paleta ─────────────────────────────────────────────────────────────
DARK = "#0f1420"
PANEL = "#1c2434"
ACCENT = "#e8b64c"   # oro macedonio
GREEN = "#2ecc71"
RED = "#e05252"
BLUE = "#4a9eff"
TEXT = "#e8ecf4"
MUTED = "#8892a6"

LOGO = f"""<svg xmlns="http://www.w3.org/2000/svg" width="160" height="160" viewBox="0 0 160 160">
  <defs>
    <radialGradient id="bg" cx="50%" cy="40%" r="70%">
      <stop offset="0%" stop-color="#1e2a44"/>
      <stop offset="100%" stop-color="{DARK}"/>
    </radialGradient>
  </defs>
  <circle cx="80" cy="80" r="76" fill="url(#bg)"/>
  <circle cx="80" cy="80" r="76" fill="none" stroke="{ACCENT}" stroke-width="2.5"/>
  <circle cx="80" cy="80" r="66" fill="none" stroke="{ACCENT}" stroke-width="0.8" stroke-dasharray="3 5"/>
  <!-- laurel izquierdo -->
  <path d="M28 95 Q40 70 56 62 M30 100 Q44 84 60 74 M34 106 Q46 94 58 86"
        stroke="{GREEN}" stroke-width="2" fill="none" stroke-linecap="round"/>
  <!-- laurel derecho -->
  <path d="M132 95 Q120 70 104 62 M130 100 Q116 84 100 74 M126 106 Q114 94 102 86"
        stroke="{GREEN}" stroke-width="2" fill="none" stroke-linecap="round"/>
  <!-- rays -->
  <g stroke="{ACCENT}" stroke-width="1.2" opacity="0.55">
    <line x1="80" y1="14" x2="80" y2="30"/>
    <line x1="80" y1="130" x2="80" y2="146"/>
    <line x1="14" y1="80" x2="30" y2="80"/>
    <line x1="130" y1="80" x2="146" y2="80"/>
    <line x1="33" y1="33" x2="44" y2="44"/>
    <line x1="116" y1="116" x2="127" y2="127"/>
    <line x1="127" y1="33" x2="116" y2="44"/>
    <line x1="44" y1="116" x2="33" y2="127"/>
  </g>
  <text x="80" y="74" text-anchor="middle" font-family="Georgia, serif" font-size="30" fill="{TEXT}" font-weight="bold">ΑΛΕΞ</text>
  <text x="80" y="100" text-anchor="middle" font-family="Georgia, serif" font-size="30" fill="{TEXT}" font-weight="bold">ΑΝΔΡΕΙΑ</text>
  <text x="80" y="122" text-anchor="middle" font-family="sans-serif" font-size="9" fill="{ACCENT}" letter-spacing="3">AUTONOMOUS ENGINE</text>
  <circle cx="80" cy="80" r="4" fill="{ACCENT}"/>
</svg>
"""

ARCH = f"""<svg xmlns="http://www.w3.org/2000/svg" width="960" height="520" viewBox="0 0 960 520">
  <rect width="960" height="520" fill="{DARK}"/>
  <text x="480" y="38" text-anchor="middle" font-family="sans-serif" font-size="20" font-weight="bold" fill="{TEXT}">ALEXANDRIA — System architecture</text>

  <!-- col 1: Claude Code -->
  <rect x="40" y="80" width="200" height="120" rx="10" fill="{PANEL}" stroke="{ACCENT}" stroke-width="1.5"/>
  <text x="140" y="110" text-anchor="middle" font-family="sans-serif" font-size="15" font-weight="bold" fill="{TEXT}">Claude Code</text>
  <text x="140" y="132" text-anchor="middle" font-family="sans-serif" font-size="10.5" fill="{MUTED}">hooks · statusline · MCP</text>
  <text x="140" y="150" text-anchor="middle" font-family="sans-serif" font-size="10.5" fill="{MUTED}">themes · skills · plugins</text>
  <text x="140" y="168" text-anchor="middle" font-family="monospace" font-size="10" fill="{BLUE}">alx setup → ⚡</text>
  <line x1="60" y1="185" x2="100" y2="185" stroke="{ACCENT}" stroke-width="0.6" opacity="0.4"/>

  <!-- col 2: Engine -->
  <rect x="380" y="50" width="220" height="420" rx="12" fill="{PANEL}" stroke="{BLUE}" stroke-width="1.5"/>
  <text x="490" y="80" text-anchor="middle" font-family="sans-serif" font-size="15" font-weight="bold" fill="{TEXT}">Engine (16 crates)</text>
  <g font-family="monospace" font-size="11" fill="{TEXT}">
    <rect x="400" y="95" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="112" text-anchor="middle">alx-gate · verifies</text>
    <rect x="400" y="127" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="144" text-anchor="middle">alx-critic · critiques</text>
    <rect x="400" y="159" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="176" text-anchor="middle">alx-task · decomposes</text>
    <rect x="400" y="191" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="208" text-anchor="middle">alx-harness · stages</text>
    <rect x="400" y="223" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="240" text-anchor="middle">alx-governor · cost</text>
    <rect x="400" y="255" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="272" text-anchor="middle">alx-memory · recalls</text>
    <rect x="400" y="287" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="304" text-anchor="middle">alx-mcp · protocol</text>
    <rect x="400" y="319" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="336" text-anchor="middle">alx-bench · measures</text>
    <rect x="400" y="351" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="368" text-anchor="middle">alx-night · autonomous</text>
    <rect x="400" y="383" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="400" text-anchor="middle">alx-evolve · learns</text>
    <rect x="400" y="415" width="180" height="26" rx="6" fill="{DARK}"/><text x="490" y="432" text-anchor="middle">alx-audit · doctor</text>
  </g>
  <text x="490" y="458" text-anchor="middle" font-family="sans-serif" font-size="10" fill="{MUTED}">207 tests · clippy-clean</text>

  <!-- col 3: LLM chain -->
  <rect x="720" y="80" width="200" height="200" rx="10" fill="{PANEL}" stroke="{GREEN}" stroke-width="1.5"/>
  <text x="820" y="110" text-anchor="middle" font-family="sans-serif" font-size="15" font-weight="bold" fill="{TEXT}">Model chain</text>
  <g font-family="monospace" font-size="11">
    <text x="820" y="140" text-anchor="middle" fill="{TEXT}">headroom :8788</text>
    <text x="820" y="160" text-anchor="middle" fill="{MUTED}">compress</text>
    <text x="820" y="186" text-anchor="middle" fill="{TEXT}">cc-model-mask :3460</text>
    <text x="820" y="206" text-anchor="middle" fill="{MUTED}">route model</text>
    <text x="820" y="232" text-anchor="middle" fill="{TEXT}">routatic :3456</text>
    <text x="820" y="252" text-anchor="middle" fill="{MUTED}">deepseek-v4-flash</text>
    <text x="820" y="272" text-anchor="middle" fill="{ACCENT}" font-size="10">model-agnostic</text>
  </g>

  <!-- arrows CC → engine -->
  <path d="M240 130 C 300 130, 320 150, 380 160" stroke="{ACCENT}" stroke-width="2" fill="none" marker-end="url(#arr)"/>
  <text x="300" y="118" text-anchor="middle" font-size="10" fill="{MUTED}" font-family="sans-serif">hooks / MCP</text>
  <path d="M240 165 C 300 200, 320 250, 380 270" stroke="{ACCENT}" stroke-width="2" fill="none" opacity="0.7"/>
  <text x="295" y="218" text-anchor="middle" font-size="10" fill="{MUTED}" font-family="sans-serif">statusline</text>

  <!-- arrows engine → chain -->
  <path d="M600 200 C 650 200, 670 180, 720 170" stroke="{GREEN}" stroke-width="2" fill="none" marker-end="url(#arr)"/>
  <text x="655" y="192" text-anchor="middle" font-size="10" fill="{MUTED}" font-family="sans-serif">LLM calls</text>

  <!-- verify loop -->
  <path d="M600 280 C 650 340, 650 380, 600 430 M490 470 L 490 460" stroke="{RED}" stroke-width="1.5" fill="none" stroke-dasharray="5 4"/>
  <text x="700" y="400" text-anchor="middle" font-size="10" fill="{RED}" font-family="sans-serif">verify → critique → improve (R24)</text>

  <defs>
    <marker id="arr" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto">
      <path d="M0 0 L10 4 L0 8 Z" fill="{ACCENT}"/>
    </marker>
  </defs>
</svg>
"""

LOOP = f"""<svg xmlns="http://www.w3.org/2000/svg" width="760" height="220" viewBox="0 0 760 220">
  <rect width="760" height="220" fill="{DARK}"/>
  <text x="380" y="30" text-anchor="middle" font-family="sans-serif" font-size="17" font-weight="bold" fill="{TEXT}">Iteration loop (R24) — the core</text>
  <g font-family="sans-serif" font-size="12" font-weight="bold">
    <rect x="40" y="80" width="140" height="60" rx="10" fill="{PANEL}" stroke="{ACCENT}" stroke-width="1.5"/>
    <text x="110" y="116" text-anchor="middle" fill="{TEXT}">Work unit</text>
    <rect x="240" y="80" width="140" height="60" rx="10" fill="{PANEL}" stroke="{GREEN}" stroke-width="1.5"/>
    <text x="310" y="116" text-anchor="middle" fill="{TEXT}">Verify (real)</text>
    <rect x="440" y="80" width="140" height="60" rx="10" fill="{PANEL}" stroke="{BLUE}" stroke-width="1.5"/>
    <text x="510" y="116" text-anchor="middle" fill="{TEXT}">Critique</text>
    <rect x="610" y="80" width="120" height="60" rx="10" fill="{PANEL}" stroke="{RED}" stroke-width="1.5"/>
    <text x="670" y="116" text-anchor="middle" fill="{TEXT}">Commit</text>
  </g>
  <path d="M180 110 L 235 110" stroke="{ACCENT}" stroke-width="2" marker-end="url(#a)"/>
  <path d="M380 110 L 435 110" stroke="{ACCENT}" stroke-width="2" marker-end="url(#a)"/>
  <path d="M580 110 L 605 110" stroke="{ACCENT}" stroke-width="2" marker-end="url(#a)"/>
  <path d="M585 140 C 440 190, 250 190, 95 140" stroke="{GREEN}" stroke-width="2" fill="none" stroke-dasharray="6 4" marker-end="url(#a)"/>
  <text x="310" y="190" text-anchor="middle" font-size="11" fill="{MUTED}">not done → improve → re-verify (state auto-advances)</text>
  <text x="670" y="165" text-anchor="middle" font-size="10.5" fill="{MUTED}">iter+1</text>
  <defs>
    <marker id="a" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto">
      <path d="M0 0 L10 4 L0 8 Z" fill="{ACCENT}"/>
    </marker>
  </defs>
</svg>
"""

for name, content in [("logo.svg", LOGO), ("architecture.svg", ARCH), ("iterate-loop.svg", LOOP)]:
    path = os.path.join(ROOT, name)
    with open(path, "w") as f:
        f.write(content)
    print(f"✓ {path} ({len(content)} bytes)")