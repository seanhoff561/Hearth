# Balance layer and realism presets

*Status: implemented (V2-0). Code: `hearth_content::Balance`. Data: `data/hearth/balance/`.*

## Purpose
Every tuning multiplier goes through one place so realism can be dialled without code changes
(v2 §3.4).

## Model
- `balance/keys.ron`: named multipliers with description, default, min and max (hunger, thirst,
  fatigue, cold/heat stress, injury severity, healing, illness chance, predator aggression,
  animal awareness, process duration, growth, yields, spoilage, tool wear, insight, carry
  capacity, excavation volume).
- `balance/presets.ron`: **Authentic** (all defaults: real-world proportioned within the time
  scales) and **Hardy** (same systems, more forgiving). **Custom** is a preset plus per-key
  overrides stored in the world settings (`Realism { preset, overrides }`).
- `Balance::resolve(content, preset, overrides)` clamps every value to its key's range;
  systems read `balance.get("hunger_rate")`.

## Interactions
Read by every simulation system. The Predator Behavior world setting (Authentic / Wild /
Tranquil) scales `predator_aggression` on top of the preset.

## Future
World-creation UI exposes each key as a slider under Custom (V2-15).
