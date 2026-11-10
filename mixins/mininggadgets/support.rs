// Game-side glue for Mining Gadgets slice 1 (injected into `crate::game` as
// `mininggadgets_state`; it needs the game, so it is not part of the
// standalone library).
//
// Upstream's `MiningGadget` is an `Item` whose `use` starts a held use that
// ray-casts and works the aimed plane through a `renderblock` ghost block, and
// whose FE lives in the stack's `mininggadgets:forge_energy` component behind
// `Capabilities.Energy.ITEM`. This slice keeps the pure arithmetic in
// `runtime.rs` and does the world work here: the FE buffer the engine keeps in
// the stack's `minecraft:custom_data` (API 307), the break through
// `native_machine_destroy_block` (API @API:machine_breaking@) with the drops a
// player's own break rolls, and the commands the in-game check drives.

use std::sync::Arc;

use horizon_client::blocks::Fluid;
use horizon_client::game::ItemEnergyBuffer;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::{JavaString, Tag};

use crate::game::mininggadgets::{
    Candidate, MAX_ENERGY, MIN_BEAM_RANGE, Pos, SizeMode, UpgradeEntry, active, affordable,
    bar_damage, break_ticks, clamp_range, collect, cost_per_block, face_of, facing_from_yaw, find,
    is_valid, known_upgrade, max_range, reserve, tier,
};
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The three gadget tiers of the jar, all the same item class upstream.
const GADGETS: [&str; 3] = [
    "mininggadgets:mininggadget",
    "mininggadgets:mininggadget_fancy",
    "mininggadgets:mininggadget_simple",
];
/// The gadget's upgrade list, as `UpgradeTools` keeps `UPGRADE_DATA`.
const UPGRADES: &str = "mininggadgets:upgrades";
/// The gadget's chosen mining range (`MiningProperties.RANGE`).
const RANGE: &str = "mininggadgets:range";
/// One upgrade entry's name.
const NAME: &str = "name";
/// Whether the player left that upgrade active.
const ACTIVE: &str = "active";

fn is_gadget(item: &str) -> bool {
    GADGETS.contains(&item)
}

/// The upgrades a gadget stack carries, in the order they were installed.
fn read_upgrades(stack: &ItemStack) -> Vec<UpgradeEntry> {
    let Some(Tag::List(entries)) = stack.data().and_then(|data| data.other(UPGRADES)) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let Tag::Compound(fields) = entry else {
                return None;
            };
            let Tag::String(name) = fields.get(&JavaString::from(NAME))? else {
                return None;
            };
            let active = !matches!(fields.get(&JavaString::from(ACTIVE)), Some(Tag::Byte(0)));
            Some(UpgradeEntry {
                name: name.to_string_lossy(),
                active,
            })
        })
        .collect()
}

/// Write the upgrade list back, as `UpgradeTools.setUpgradesNBT` lays it out.
fn write_upgrades(stack: &mut ItemStack, upgrades: &[UpgradeEntry]) {
    let list: Vec<Tag> = upgrades
        .iter()
        .map(|entry| {
            Tag::compound([
                (NAME, Tag::String(JavaString::from(entry.name.as_str()))),
                (ACTIVE, Tag::Byte(i8::from(entry.active))),
            ])
        })
        .collect();
    let value = (!list.is_empty()).then_some(Tag::List(list));
    stack.data_mut().set_other(UPGRADES, value);
    stack.tidy();
}

/// The mining range the gadget carries, defaulting to 1 as upstream's does.
fn read_range(stack: &ItemStack) -> i32 {
    match stack.data().and_then(|data| data.other(RANGE)) {
        Some(Tag::Int(range)) => *range,
        _ => 1,
    }
}

fn write_range(stack: &mut ItemStack, range: i32) {
    let value = (range != 1).then_some(Tag::Int(range));
    stack.data_mut().set_other(RANGE, value);
    stack.tidy();
}

/// The FE one item of `stack` holds (`Capabilities.Energy.ITEM`, API 307).
fn energy_of(game: &crate::game::Game<'_>, stack: &ItemStack) -> i32 {
    game.native_item_energy(stack)
        .map_or(0, |held| held.amount.clamp(0, MAX_ENERGY))
}

/// The acting player's held stack, as the commands read it.
fn held(game: &crate::game::Game<'_>) -> ItemStack {
    game.held()
        .cloned()
        .unwrap_or_else(|| ItemStack::new("minecraft:air", 0))
}

/// Run `change` on one item of the acting player's held stack and put the
/// result back, with the hand told the stack changed.
fn change_held(
    game: &mut crate::game::Game<'_>,
    change: impl FnOnce(&mut crate::game::Game<'_>, &ItemStack) -> Option<ItemStack>,
) -> bool {
    let slot = game.held_slot();
    let Some(stack) = game.stack_in_mut(slot).take() else {
        return false;
    };
    let changed = change(game, &stack);
    *game.stack_in_mut(slot) = Some(changed.unwrap_or(stack));
    game.hand_stack_replaced(slot);
    true
}

/// Charge or discharge the held gadget to `target` FE.
fn set_energy(game: &mut crate::game::Game<'_>, target: i32) -> Option<i32> {
    let target = target.clamp(0, MAX_ENERGY);
    let changed = change_held(game, |game, stack| {
        let current = energy_of(game, stack);
        let mut one = stack.clone();
        if target > current
            && let Some((_, charged)) = game.native_item_energy_insert(stack, target - current)
        {
            one = charged;
        } else if target < current
            && let Some((_, drained)) = game.native_item_energy_extract(stack, current - target)
        {
            one = drained;
        }
        one.damage = bar_damage(target);
        Some(one)
    });
    changed.then_some(target)
}

/// Pay `cost` FE out of the held gadget; false when it cannot.
fn spend(game: &mut crate::game::Game<'_>, cost: i32) -> bool {
    if cost <= 0 {
        return true;
    }
    let slot = game.held_slot();
    let Some(stack) = game.stack_in_mut(slot).take() else {
        return false;
    };
    match game
        .native_item_energy_extract(&stack, cost)
        .filter(|(gave, _)| *gave >= cost)
    {
        Some((_, mut one)) => {
            one.damage = bar_damage(energy_of(game, &one));
            *game.stack_in_mut(slot) = Some(one);
            game.hand_stack_replaced(slot);
            true
        }
        None => {
            *game.stack_in_mut(slot) = Some(stack);
            false
        }
    }
}

/// Give `cost` FE back to the acting player's gadget, when a break it paid for
/// did not happen.
fn refund(game: &mut crate::game::Game<'_>, cost: i32) {
    if cost <= 0 {
        return;
    }
    let current = energy_of(game, &held(game));
    let _ = set_energy(game, current.saturating_add(cost));
}

/// The block at `pos`, as the pure target rules take it.
fn candidate(game: &crate::game::Game<'_>, pos: Pos) -> Candidate {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts().get(state);
    Candidate {
        air: game.res.facts().is_air(state),
        fluid: !matches!(facts.fluid, Fluid::None),
        waterlogged: facts.waterlogged,
        hardness: facts.hardness,
        block_entity: facts.block_entity,
        door: facts.classes.iter().any(|class| &**class == "DoorBlock"),
    }
}

/// `MiningGadget.onUseTick`'s server half, fired once per right click: collect
/// the aimed plane, drop what the game will not let a player break, break what
/// the buffer pays for, and start the item's cooldown for the dig time.
fn fire(game: &mut crate::game::Game<'_>, stack: &ItemStack, aimed: Option<(Pos, Pos)>) {
    let Some((start, face)) = aimed else {
        return;
    };
    let upgrades = read_upgrades(stack);
    let range = clamp_range(read_range(stack).min(max_range(&upgrades)));
    let facing = facing_from_yaw(f64::from(game.yaw));
    let player_y = game.player.as_ref().map_or(f64::NAN, |body| body.position.y);
    let targets: Vec<(Pos, f32)> = collect(start, face, facing, range, SizeMode::Auto, player_y)
        .into_iter()
        .filter_map(|pos| {
            let candidate = candidate(game, pos);
            is_valid(&candidate).then_some((pos, candidate.hardness.max(0.0)))
        })
        .collect();
    if targets.is_empty() {
        return;
    }
    let energy = energy_of(game, stack);
    if energy < reserve(&upgrades, range) {
        return;
    }
    let cost = cost_per_block(&upgrades);
    let paid = affordable(energy, cost, targets.len());
    // The drops are rolled with a gadget stack carrying the upgrades' own
    // enchantments, as upstream builds `tempTool` in `RenderBlockTileEntity`.
    let mut tool = stack.clone();
    if active(&upgrades, "silk") {
        tool.enchant("minecraft:silk_touch", 1);
    }
    let fortune = tier(&upgrades, "fortune_1");
    if fortune > 0 {
        tool.enchant("minecraft:fortune", fortune as u32);
    }
    let mut broken = 0;
    let mut hardness = Vec::with_capacity(paid);
    for (pos, of_block) in targets.iter().take(paid) {
        if !spend(game, cost) {
            break;
        }
        if game
            .native_machine_destroy_block(*pos, Some(&tool), false)
            .is_none()
        {
            // The world refused the break (an unloaded chunk, or off a
            // server): nothing happened, so nothing is paid.
            refund(game, cost);
            continue;
        }
        hardness.push(*of_block);
        broken += 1;
    }
    if broken == 0 {
        return;
    }
    let haste = game.potions.effects.amplifier("haste").unwrap_or(0);
    let fatigue = game
        .potions
        .effects
        .amplifier("mining_fatigue")
        .is_some();
    let ticks = break_ticks(&hardness, range, tier(&upgrades, "efficiency_1"), haste, fatigue);
    game.add_cooldown(Arc::clone(&stack.item), ticks);
}

/// Right click on a block (`native_use_item_on`, API 13): the block's own use
/// does not run, as upstream cancels `RightClickBlock` for a held gadget.
#[allow(private_interfaces, reason = "the injected bundle is this crate's own module")]
pub fn on_use_item_on(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    pos: Pos,
    face: Pos,
    claimed: &mut bool,
) {
    if !is_gadget(&stack.item) {
        return;
    }
    *claimed = true;
    fire(game, stack, Some((pos, face)));
}

/// Right click in the air (`native_use_item`, API 13): ray-cast to the beam
/// range, then fire at the face the ray met.
#[allow(private_interfaces, reason = "the injected bundle is this crate's own module")]
pub fn on_use_item(game: &mut crate::game::Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if !is_gadget(&stack.item) {
        return;
    }
    *claimed = true;
    let Some((distance, hit)) = game.native_view_hit() else {
        return;
    };
    if distance > MIN_BEAM_RANGE as f32 {
        return;
    }
    let block = [hit.block_x, hit.block_y, hit.block_z];
    let face = face_of([hit.x, hit.y, hit.z], block);
    fire(game, stack, Some((block, face)));
}

/// The component ids the gadget's own items name in their content JSON.
pub fn on_component_types(types: &mut Vec<String>) {
    types.push(UPGRADES.to_owned());
    types.push(RANGE.to_owned());
}

/// `Capabilities.Energy.ITEM` for the gadget: the buffer upstream builds an
/// `EnergyStorageItemstack` with, 1,000,000 FE, and unlimited insert/extract
/// per call as upstream's `Integer.MAX_VALUE` limits are.
pub fn on_item_energy_buffer(item: &str, buffer: &mut Option<ItemEnergyBuffer>) {
    if is_gadget(item) {
        *buffer = Some(ItemEnergyBuffer {
            capacity: MAX_ENERGY,
            max_insert: MAX_ENERGY,
            max_extract: MAX_ENERGY,
        });
    }
}

/// The gadget's tooltip: its charge, its per-block price and its range.
#[allow(private_interfaces, reason = "the injected bundle is this crate's own module")]
pub fn on_tooltip(game: &crate::game::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if !is_gadget(&stack.item) {
        return;
    }
    let upgrades = read_upgrades(stack);
    let energy = energy_of(game, stack);
    let range = clamp_range(read_range(stack).min(max_range(&upgrades)));
    lines.push((
        Text::tr("mininggadgets.gadget.energy", &[&energy, &MAX_ENERGY]),
        0x55FF55,
    ));
    lines.push((
        Text::tr(
            "mininggadgets.tooltip.item.break_cost",
            &[&cost_per_block(&upgrades)],
        ),
        0xFF5555,
    ));
    lines.push((
        Text::tr("mininggadgets.gadget.range_change", &[&range]),
        0xAAAAAA,
    ));
}

/// `/mininggadgets info|energy|range|upgrade`.
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("mininggadgets info"));
    commands.push(NativeCommand::op(
        "mininggadgets energy <amount:integer>",
        2,
    ));
    commands.push(NativeCommand::op("mininggadgets range <size:integer>", 2));
    commands.push(NativeCommand::op("mininggadgets upgrade <name:word>", 2));
}

/// The names `/mininggadgets info` lists for a gadget.
fn upgrades_summary(upgrades: &[UpgradeEntry]) -> String {
    if upgrades.is_empty() {
        return "empty".to_owned();
    }
    upgrades
        .iter()
        .map(|entry| {
            if entry.active {
                entry.name.clone()
            } else {
                format!("{} (off)", entry.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Run a `/mininggadgets` command a player typed in chat.
#[allow(private_interfaces, reason = "the injected bundle is this crate's own module")]
pub fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["mininggadgets", "info"] => {
            *ran = true;
            let stack = held(game);
            if !is_gadget(&stack.item) {
                game.native_feedback(Text::literal("Hold a Mining Gadget first."));
                return;
            }
            let upgrades = read_upgrades(&stack);
            game.native_feedback(Text::literal(format!(
                "Mining Gadget: {}/{} FE, per block {}, range {} of {}, upgrades {}, t={}",
                energy_of(game, &stack),
                MAX_ENERGY,
                cost_per_block(&upgrades),
                clamp_range(read_range(&stack).min(max_range(&upgrades))),
                max_range(&upgrades),
                upgrades_summary(&upgrades),
                game.world.game_time
            )));
        }
        ["mininggadgets", "energy", amount] => {
            *ran = true;
            let Ok(amount) = amount.parse::<i32>() else {
                game.native_feedback(Text::literal("Usage: /mininggadgets energy <amount>"));
                return;
            };
            if !is_gadget(&held(game).item) {
                game.native_feedback(Text::literal("Hold a Mining Gadget first."));
                return;
            }
            match set_energy(game, amount) {
                Some(energy) => game.native_feedback(Text::literal(format!(
                    "Mining Gadget energy set to {energy}/{MAX_ENERGY} FE."
                ))),
                None => game.native_feedback(Text::literal("The charge could not be set.")),
            }
        }
        ["mininggadgets", "range", size] => {
            *ran = true;
            let Ok(size) = size.parse::<i32>() else {
                game.native_feedback(Text::literal("Usage: /mininggadgets range <size>"));
                return;
            };
            let stack = held(game);
            if !is_gadget(&stack.item) {
                game.native_feedback(Text::literal("Hold a Mining Gadget first."));
                return;
            }
            let allowed = max_range(&read_upgrades(&stack));
            let range = clamp_range(size).min(allowed);
            change_held(game, |_, stack| {
                let mut one = stack.clone();
                write_range(&mut one, range);
                Some(one)
            });
            game.native_feedback(Text::literal(format!(
                "Mining Gadget range set to {range} of {allowed}."
            )));
        }
        ["mininggadgets", "upgrade", name] => {
            *ran = true;
            let stack = held(game);
            if !is_gadget(&stack.item) {
                game.native_feedback(Text::literal("Hold a Mining Gadget first."));
                return;
            }
            let mut upgrades = read_upgrades(&stack);
            if *name == "clear" {
                upgrades.clear();
            } else if find(&upgrades, name).is_some() {
                game.native_feedback(Text::literal(format!(
                    "The gadget already has {name}."
                )));
                return;
            } else if !known_upgrade(name) {
                game.native_feedback(Text::literal(format!("{name} is not an upgrade.")));
                return;
            } else {
                upgrades.push(UpgradeEntry::new(name));
            }
            // The chosen range never exceeds what the new size allows.
            let range = clamp_range(read_range(&stack).min(max_range(&upgrades)));
            change_held(game, |_, stack| {
                let mut one = stack.clone();
                write_upgrades(&mut one, &upgrades);
                write_range(&mut one, range);
                Some(one)
            });
            game.native_feedback(Text::literal(format!(
                "Mining Gadget upgrades: {}, per block {}, range {range} of {}.",
                upgrades_summary(&upgrades),
                cost_per_block(&upgrades),
                max_range(&upgrades)
            )));
        }
        _ => {}
    }
}
