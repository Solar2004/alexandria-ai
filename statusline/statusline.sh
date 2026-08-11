#!/bin/bash
# ============================================================
#  statusline.sh — barra de datos de AlexanderTheGreat
#  Claude Code envía JSON por stdin; imprimimos una línea con:
#  ☀/☾ (según hora) · hora · modelo · contexto % · coste $
#  · git branch · RAM. Colores: paleta SOL (día) / LUNA (noche).
#  Instalar: settings.json → statusLine.command
# ============================================================
input=$(cat)
HOUR=$(date +%H)

# --- Paleta SOL (06:00-18:59): dorado/ámbar/crema — luna y sol ---
# --- Paleta LUNA (19:00-05:59): índigo/plata sobre negro ---
if [ "$HOUR" -ge 6 ] && [ "$HOUR" -lt 19 ]; then
  ICON="☀"
  C_ICON=$'\033[38;2;255;176;32m'      # ámbar
  C_TXT=$'\033[38;2;255;248;231m'      # crema
  C_DIM=$'\033[38;2;210;170;110m'      # dorado apagado
  C_BAR=$'\033[38;2;255;215;0m'        # oro
  C_WARN=$'\033[38;2;230;126;34m'      # naranja
  C_RESET=$'\033[0m'
else
  ICON="☾"
  C_ICON=$'\033[38;2;138;155;255m'     # índigo claro
  C_TXT=$'\033[38;2;192;192;224m'      # plata
  C_DIM=$'\033[38;2;120;130;180m'      # índigo apagado
  C_BAR=$'\033[38;2;123;104;238m'      # medium slate blue
  C_WARN=$'\033[38;2;255;99;132m'      # rosa
  C_RESET=$'\033[0m'
fi

# --- Extracción con jq (defaults seguros) ---
model=$(echo "$input" | jq -r '.model.display_name // .model.id // "model?"' 2>/dev/null)
ctx=$(echo "$input" | jq -r '.context_window.used_percentage // 0' 2>/dev/null)
cost=$(echo "$input" | jq -r '.cost.total_cost_usd // 0' 2>/dev/null)
branch=$(echo "$input" | jq -r '.git.branch // ""' 2>/dev/null)
dirty=$(echo "$input" | jq -r '.git.status | length' 2>/dev/null)
[ -z "$dirty" ] || [ "$dirty" = "null" ] && dirty=0

# --- Barra de contexto (10 segmentos) ---
segs=10
filled=$(( ctx * segs / 100 ))
[ "$filled" -gt "$segs" ] && filled=$segs
bar=""
for ((i=0; i<segs; i++)); do
  if [ "$i" -lt "$filled" ]; then bar+="▓"; else bar+="░"; fi
done

# --- RAM ---
if [ -r /proc/meminfo ]; then
  memtot=$(awk '/MemTotal/{print $2}' /proc/meminfo)
  memavail=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
  memused=$(( (memtot - memavail) / 1024 / 1024 ))
  ram="${memused}G"
else
  ram=""
fi

# --- Coste (solo si > 0.001) ---
cost_txt=""
if [ "$(echo "$cost" | awk '{print ($1>0.001)?1:0}')" = "1" ]; then
  cost_txt=" ${C_WARN}\$$(echo "$cost" | awk '{printf "%.3f", $1}')${C_RESET}"
fi

# --- Git ---
git_txt=""
if [ -n "$branch" ] && [ "$branch" != "null" ]; then
  dirty_txt=""
  [ "$dirty" -gt 0 ] && dirty_txt="${C_WARN}+${dirty}${C_RESET}"
  git_txt=" ${C_DIM}⎇${C_RESET} ${branch}${dirty_txt}"
fi

echo -n "${C_ICON}${ICON}${C_RESET} ${C_TXT}$(date +%H:%M)${C_RESET}"
echo -n " ${C_DIM}│${C_RESET} ${model}"
echo -n " ${C_DIM}│${C_RESET} ${C_BAR}${bar}${C_RESET} ${ctx}%"
echo -n "${cost_txt}"
echo -n "${git_txt}"
[ -n "$ram" ] && echo -n " ${C_DIM}│${C_RESET} ${ram}"
echo ""
