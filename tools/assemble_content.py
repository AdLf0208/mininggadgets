#!/usr/bin/env python3
"""Assemble the slice-1 `content/` folder from the pack's Mining Gadgets jar.

The jar is the ATM10 file (`ports/_shared/licence-cache/mininggadgets-1.18.8.jar`,
SHA-1 `2f97c2d5…ad30` = the pack's `modrinthHash`), kept as the licence and
provenance evidence. Nothing is invented: every model, texture, recipe, lang file
and name comes out of the jar, and the only decisions are listed in
`content/README.md`.

Rerun it to rebuild `content/` (it deletes its own generated folders first):

    python3 tools/assemble_content.py
"""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/mininggadgets-1.18.8.jar"
CONTENT = ROOT / "content"
MOD = "mininggadgets"

# The three gadget tiers (`Registration.MININGGADGET*`), all the same item class.
GADGETS = ["mininggadget", "mininggadget_fancy", "mininggadget_simple"]
# `UpgradeCard` items and the upgrade each one carries, from `Registration`.
# `upgrade_empty` carries no upgrade and stacks to 64, the rest to 1.
UPGRADES = {
    "upgrade_empty": None,
    "upgrade_silk": "silk",
    "upgrade_void_junk": "void_junk",
    "upgrade_magnet": "magnet",
    "upgrade_size_1": "size_1",
    "upgrade_size_2": "size_2",
    "upgrade_size_3": "size_3",
    "upgrade_light_placer": "light_placer",
    "upgrade_freezing": "freezing",
    "upgrade_fortune_1": "fortune_1",
    "upgrade_fortune_2": "fortune_2",
    "upgrade_fortune_3": "fortune_3",
    "upgrade_range_1": "range_1",
    "upgrade_range_2": "range_2",
    "upgrade_range_3": "range_3",
    "upgrade_battery_1": "battery_1",
    "upgrade_battery_2": "battery_2",
    "upgrade_battery_3": "battery_3",
    "upgrade_battery_creative": "battery_creative",
    "upgrade_efficiency_1": "efficiency_1",
    "upgrade_efficiency_2": "efficiency_2",
    "upgrade_efficiency_3": "efficiency_3",
    "upgrade_efficiency_4": "efficiency_4",
    "upgrade_efficiency_5": "efficiency_5",
}
# The gadget's own FE buffer, `Config.MININGGADGET_MAXPOWER`, mirrored onto the
# bar (`MiningGadget.getMaxDamage` returns the same number).
MAX_ENERGY = 1_000_000
# The convention tags the jar's recipes use; the prepared 26.3 root ships none
# of them, and an unknown tag expands to an empty ingredient.
CONVENTION_TAGS = {
    "dusts/redstone": ["minecraft:redstone"],
    "ender_pearls": ["minecraft:ender_pearl"],
    "gems/diamond": ["minecraft:diamond"],
    "gems/lapis": ["minecraft:lapis_lazuli"],
    "glass_panes": ["minecraft:glass_pane"]
    + [
        f"minecraft:{colour}_stained_glass_pane"
        for colour in (
            "white",
            "orange",
            "magenta",
            "light_blue",
            "yellow",
            "lime",
            "pink",
            "gray",
            "light_gray",
            "cyan",
            "purple",
            "blue",
            "brown",
            "green",
            "red",
            "black",
        )
    ],
    "ingots/gold": ["minecraft:gold_ingot"],
    "ingots/iron": ["minecraft:iron_ingot"],
    "nether_stars": ["minecraft:nether_star"],
    "slimeballs": ["minecraft:slime_ball"],
}


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def upgrade_recipe(recipe: dict) -> dict:
    """1.21.1 shaped recipes to 26.3: `{"item"|"tag": x}` becomes `"x"|"#x"`."""
    key = {}
    for name, ingredient in recipe.get("key", {}).items():
        if isinstance(ingredient, str):
            key[name] = ingredient
        elif "item" in ingredient:
            key[name] = ingredient["item"]
        elif "tag" in ingredient:
            key[name] = f"#{ingredient['tag']}"
        else:
            raise SystemExit(f"unknown ingredient {ingredient!r}")
    upgraded = dict(recipe, key=key)
    result = upgraded.get("result")
    if isinstance(result, dict) and "item" in result:
        result = dict(result, id=result.pop("item"))
        upgraded["result"] = result
    return upgraded


def main() -> None:
    if not JAR.is_file():
        raise SystemExit(f"missing the pack jar: {JAR}")
    for folder in ("assets", "data"):
        shutil.rmtree(CONTENT / folder, ignore_errors=True)
    with zipfile.ZipFile(JAR) as jar:
        names = set(jar.namelist())

        def read(path: str) -> bytes:
            return jar.read(path)

        # 1. Item models, straight from the jar, plus the 26.3 model definition
        #    that names them (`assets/<id>/items/<name>.json`).
        for name in [*GADGETS, *UPGRADES]:
            model = f"assets/{MOD}/models/item/{name}.json"
            if model not in names:
                raise SystemExit(f"the jar has no {model}")
            (CONTENT / model).parent.mkdir(parents=True, exist_ok=True)
            (CONTENT / model).write_bytes(read(model))
            write_json(
                CONTENT / f"assets/{MOD}/items/{name}.json",
                {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{name}"}},
            )

        # 2. The textures those models name, and the animated-sprite sidecars.
        textures = sorted(
            n
            for n in names
            if not n.endswith("/")
            and (
                n.startswith(f"assets/{MOD}/textures/item/gadget/")
                or (n.startswith(f"assets/{MOD}/textures/item/upgrade_") and ".png" in n)
            )
        )
        for texture in textures:
            (CONTENT / texture).parent.mkdir(parents=True, exist_ok=True)
            (CONTENT / texture).write_bytes(read(texture))

        # 3. The jar's own names.
        lang = f"assets/{MOD}/lang/en_us.json"
        (CONTENT / lang).parent.mkdir(parents=True, exist_ok=True)
        (CONTENT / lang).write_bytes(read(lang))

        # 4. Recipes: the three gadgets and every upgrade card except the
        #    modification table (that block is not in this slice).
        for path in sorted(names):
            if not path.startswith(f"data/{MOD}/recipe/") or path.endswith("/"):
                continue
            name = Path(path).stem
            if name == "modificationtable":
                continue
            if name not in {*GADGETS, *UPGRADES}:
                raise SystemExit(f"unexpected recipe {name}")
            recipe = upgrade_recipe(json.loads(read(path)))
            write_json(CONTENT / f"data/{MOD}/recipe/{name}.json", recipe)

    # 5. What the content declares: the items, their components, the tab.
    for name in GADGETS:
        write_json(
            CONTENT / f"data/{MOD}/horizon/item/{name}.json",
            {
                "template": "minecraft:stick",
                "max_stack_size": 1,
                "components": {
                    "minecraft:max_damage": MAX_ENERGY,
                    "!minecraft:repairable": {},
                },
            },
        )
    # Upstream's upgrade cards carry no data component: the item *is* the
    # upgrade (`UpgradeCard.upgrade` is a Java field, and the id names it), so a
    # card needs no native component and the `.hmod` loads on its own.
    for name in UPGRADES:
        write_json(
            CONTENT / f"data/{MOD}/horizon/item/{name}.json",
            {
                "template": "minecraft:paper",
                "max_stack_size": 64 if name == "upgrade_empty" else 1,
                "components": {},
            },
        )
    write_json(
        CONTENT / f"data/{MOD}/horizon/creative_tab/main.json",
        {
            "title": "itemGroup.mininggadgets",
            "icon": f"{MOD}:mininggadget",
            "items": [*[f"{MOD}:{name}" for name in GADGETS], *[f"{MOD}:{name}" for name in UPGRADES]],
        },
    )

    # 6. The convention tags the recipes name.
    for tag, values in CONVENTION_TAGS.items():
        write_json(CONTENT / f"data/c/tags/item/{tag}.json", {"values": values})

    print(f"assembled {sum(1 for _ in CONTENT.rglob('*') if _.is_file())} content files")


if __name__ == "__main__":
    main()
