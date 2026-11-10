# Mining Gadgets — Horizon port

A port of [Mining Gadgets](https://github.com/Direwolf20-MC/MiningGadgets) by
Direwolf20 and ErrorMikey for Horizon. This slice is the gadget itself: the
right-click laser and the plane of blocks it clears.

- Right-click a block, or the air in front of you, with a gadget: it mines the
  aimed face's plane (wider ranges are the square on that plane, oriented by the
  face) and nothing it is not allowed to break — no block entities, doors or
  fluids other than waterlogged blocks.
- Each broken block costs FE: 200 base plus the active upgrades' cost per block.
  A shot needs `cost × range × range` in the buffer before it starts. The gadget
  holds 1,000,000 FE and its damage bar is the charge.
- The three tiers (MK1–MK3) are carried as items, and the upgrade list
  (`{name, active}`) is read and written as data.

The held-use trigger, the ghost preview, the client beam, particles and sounds,
the settings and filter screens, the Modification Table and the remaining
upgrade effects are later slices. MIT — original copyright and licence notices
are kept in the port repository; see LICENSE and LICENSE-NOTICE.md there.

**Original:** [Mining Gadgets](https://github.com/Direwolf20-MC/MiningGadgets) 1.18.8 by
Direwolf20, ErrorMikey, **MIT**. The original
copyright and licence notices are kept in [LICENSE](LICENSE), and
[LICENSE-NOTICE.md](LICENSE-NOTICE.md) records where each was read from.

**Horizon:** a port for [Horizon](https://github.com/horizon-rust/horizon), Minecraft:
Java Edition 26.3 rewritten in Rust. The exact Horizon and horizon-mixins
revisions this port builds against are pinned in `compatibility.json`.

## Install

1. Take `mininggadgets-0.1.0.hnmod` from this repository (or build it below).
2. Put it in the game folder's `horizon-native-mods` folder — the game folder is the one
   holding `options.txt`.
3. Native mods are always on once installed; the first install shows a
   trust notice, and the launcher builds the mod for your game.

To try it without installing, start the game with
`--dev-mod mininggadgets-0.1.0.hnmod`.

## Build

The port is a native mod (`.hnmod`): carried content plus a Rust runtime and
source mixins.

```sh
cargo test --locked --offline                        # the pure runtime rules
python3 tools/horizon.py stage   --horizon <horizon-checkout>
python3 tools/horizon.py check   --horizon <horizon-checkout>
python3 tools/horizon.py test    --horizon <horizon-checkout>
python3 tools/horizon.py build   --horizon <horizon-checkout>   # rebuild the .hnmod
```

`tools/e2e.sh` is the in-game harness. It expects a campaign checkout
(`$CAMPAIGN_ROOT`), the shared pins under `ports/_shared/pins/`, and the
prepared 26.3 resources (`$HORIZON_RES`, default `.local/horizon-client/26.3`).

## Layout

`content/` is the shipped content; `mixins/mininggadgets/` holds the declarations and the runtime; `tools/` builds and verifies the port.

This is an unofficial port: Horizon is not affiliated with the original author.
The licence above is the original's and travels with every copy; see
[LICENSE](LICENSE) and [LICENSE-NOTICE.md](LICENSE-NOTICE.md).
