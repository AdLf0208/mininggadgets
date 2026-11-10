# `content/` — what is carried, what is patched, what is not here

Assembled from the pack jar by [`../tools/assemble_content.py`](../tools/assemble_content.py);
rerun it to rebuild this folder. The jar is
`ports/_shared/licence-cache/mininggadgets-1.18.8.jar` (SHA-1
`2f97c2d5…ad30` = the ATM10 record; see
`ports/_shared/carry-work/mininggadgets/SOURCE.md`).

## Carried byte for byte from the jar

- `assets/mininggadgets/models/item/<name>.json` — the jar's own item models for
  the three gadget tiers and the 24 upgrade cards (the gadget models are
  Blockbench element models; the cards are `item/handheld` layers).
- `assets/mininggadgets/textures/item/gadget/**` (the three gadget texture
  atlases and their animated `.mcmeta` sidecars) and
  `assets/mininggadgets/textures/item/upgrade_*.png` — every sprite the models
  name.
- `assets/mininggadgets/lang/en_us.json` — the jar's names and tooltip keys,
  unchanged (the native half uses `mininggadgets.gadget.energy`,
  `mininggadgets.tooltip.item.break_cost` and
  `mininggadgets.gadget.range_change`).
- `data/mininggadgets/recipe/*.json` — the three gadget recipes and the 23
  upgrade-card recipes, upgraded 1.21.1 → 26.3 (ingredients `{"item": x}` /
  `{"tag": t}` become `"x"` / `"#t"`). `modificationtable.json` is left out:
  that block is not in this slice.

## Written by the assembler (and why)

- `assets/mininggadgets/items/<name>.json` — the 26.3 item-model definition
  (`{"model": {"type": "minecraft:model", "model": "mininggadgets:item/<name>"}}`),
  which 1.21.1 jars do not have. It is the only way 26.3 finds the carried model.
- `data/mininggadgets/horizon/item/mininggadget*.json` — the three gadgets as
  `minecraft:stick` with `max_stack_size: 1`, `minecraft:max_damage: 1000000`
  and `!minecraft:repairable`. `max_damage` is the energy bar: upstream's
  `getMaxDamage` returns `MAXPOWER` and `getBarWidth`/`getBarColor` read the
  FE buffer, so the native half mirrors the charge into the stack's `damage`
  (see `notes.md`).
- `data/mininggadgets/horizon/item/upgrade_*.json` — `minecraft:paper` with
  `max_stack_size` 1 (64 for `upgrade_empty`, as upstream's `UpgradeCard`
  constructor does) and no components: upstream's card carries no data, the
  item id is the upgrade.
- `data/mininggadgets/horizon/creative_tab/main.json` — upstream's tab is code;
  one tab with the three gadgets and the 24 cards.
- `data/c/tags/item/*.json` — the nine convention tags the recipes name
  (`dusts/redstone`, `ender_pearls`, `gems/diamond`, `gems/lapis`,
  `glass_panes`, `ingots/gold`, `ingots/iron`, `nether_stars`, `slimeballs`) as
  their vanilla items. The prepared 26.3 root ships no `data/c/` tags, and an
  unknown tag expands to an **empty** ingredient, so without these the recipes
  would load and never match.

## Deliberately not carried

- The blocks: `modificationtable` (the upgrade UI), `minerslight` and
  `renderblock` (the ghost block the laser draws). All three are slice-2 work
  with their block entities, menus and renderers.
- `data/mininggadgets/advancement/**` (recipe unlocks), the `projecte` tag and
  the two `minecraft` block tags that name blocks this slice does not have.
- The `olf_textures/` legacy sprite set, which no 1.18.8 model names.
