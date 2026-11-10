// Pure Mining Gadgets logic for slice 1 (standalone crate lib, injected as
// `crate::game::mininggadgets`). No game access lives here: the per-block FE
// cost of a shot, the FE a shot must reserve, the blocks a shot collects on the
// aimed face, which of those are legal targets, how long one block takes, and
// how much of a target list a buffer pays for.
//
// Upstream's `MiningGadget` holds the trigger down: every tick it ray-casts to
// `beamRange`, collects the blocks of the aimed plane (`MiningCollect.collect`),
// replaces each with a `renderblock` that counts down `floor(hardness * 30 /
// toolSpeed)`, and charges `getEnergyCost(stack)` FE once when a block reaches
// zero. Slice 1 keeps the arithmetic and the target shape, fires once per
// right-click, and charges the same per-block price.

pub type Pos = [i32; 3];

/// `Config.MININGGADGET_MAXPOWER` (default `maxPower`): the gadget's FE buffer.
pub const MAX_ENERGY: i32 = 1_000_000;
/// `Config.MININGGADGET_BASECOST` (default `baseCost`): FE for one block.
pub const BASE_COST: i32 = 200;
/// `MiningProperties.MIN_RANGE`: the default beam reach, in blocks.
pub const MIN_BEAM_RANGE: i32 = 5;
/// The gadget's own mining speed, as upstream's `getHardness` starts it.
pub const TOOL_SPEED: f32 = 8.0;
/// The largest range a `size_3` upgrade allows (a 7 x 7 plane).
pub const MAX_RANGE: i32 = 7;

/// One installed upgrade, as the gadget's `mininggadgets:upgrades` component
/// keeps it: the upgrade's own name and whether the player left it active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeEntry {
    pub name: String,
    pub active: bool,
}

impl UpgradeEntry {
    /// An active upgrade called `name`.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            active: true,
        }
    }
}

/// How a gadget's mining plane grows around the aimed block
/// (`MiningProperties.SizeMode`): `auto` flattens the plane underfoot,
/// `normal` keeps it centred on the aimed block, `pathway` digs upward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(
    dead_code,
    reason = "slice 1 always mines in Auto; the other two shapes are pure and tested"
)]
pub enum SizeMode {
    #[default]
    Auto,
    Normal,
    Pathway,
}

/// The one field of an NBT compound that decides a target (`BlockState`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Candidate {
    pub air: bool,
    pub fluid: bool,
    pub waterlogged: bool,
    pub hardness: f32,
    pub block_entity: bool,
    pub door: bool,
}

/// The base name an upgrade name belongs to: `fortune_2` is a `fortune`, and
/// `battery_creative` a `battery`, as upstream's enum builds its base names.
pub fn base_name(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((base, last))
            if !base.is_empty()
                && (last == "creative" || last.chars().all(|c| c.is_ascii_digit())) =>
        {
            base
        }
        _ => name,
    }
}

/// `Upgrade.getCostPerBlock` for an upgrade's name, in FE; 0 for a name this
/// table does not know and for the upgrades that charge somewhere else (a
/// battery, a size, a range) or charge per operation (freezing, light).
pub fn upgrade_cost(name: &str) -> i32 {
    match base_name(name) {
        "silk" => 100,
        "void_junk" => 10,
        "magnet" => 25,
        "fortune" => match &name[name.len().saturating_sub(1)..] {
            "1" => 30,
            "2" => 60,
            "3" => 100,
            _ => 30,
        },
        "efficiency" => match &name[name.len().saturating_sub(1)..] {
            "1" => 10,
            "2" => 20,
            "3" => 30,
            "4" => 40,
            "5" => 50,
            _ => 10,
        },
        _ => 0,
    }
}

/// `Upgrade.getTier`: the tier a name carries, 0 when it has none (upstream's
/// `-1`; a creative battery is tier 4).
fn upgrade_tier(name: &str) -> i32 {
    let last = match name.rsplit_once('_') {
        Some((base, last)) if base == base_name(name) => last,
        _ => return 0,
    };
    match base_name(name) {
        "size" | "fortune" | "battery" | "range" | "efficiency" => match last {
            "creative" => 4,
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            _ => 0,
        },
        _ => 0,
    }
}

/// The upgrade with the same base name, as `UpgradeTools.getUpgradeFromList`
/// matches tiered names (`fortune_2` is a `fortune_1`).
pub fn find<'a>(upgrades: &'a [UpgradeEntry], name: &str) -> Option<&'a UpgradeEntry> {
    let wanted = base_name(name);
    upgrades
        .iter()
        .find(|entry| base_name(&entry.name) == wanted)
}

/// Whether the given upgrade is installed and left active
/// (`UpgradeTools.containsActiveUpgrade`).
pub fn active(upgrades: &[UpgradeEntry], name: &str) -> bool {
    find(upgrades, name).is_some_and(|entry| entry.active)
}

/// The tier of the installed upgrade with this base name, 0 when there is none
/// (`Upgrade.getTier`; `battery_creative` is tier 4).
pub fn tier(upgrades: &[UpgradeEntry], name: &str) -> i32 {
    find(upgrades, name)
        .filter(|entry| entry.active)
        .map_or(0, |entry| upgrade_tier(&entry.name))
}

/// `MiningGadget.getEnergyCost`: the FE one broken block costs — the base plus
/// every active upgrade's per-block cost. A creative battery makes it 0.
pub fn cost_per_block(upgrades: &[UpgradeEntry]) -> i32 {
    if tier(upgrades, "battery_1") == 4 {
        return 0;
    }
    let added: i32 = upgrades
        .iter()
        .filter(|entry| entry.active)
        .map(|entry| upgrade_cost(&entry.name))
        .sum();
    BASE_COST.saturating_add(added)
}

/// `MiningGadget.canMine`'s reserve: a shot with `range > 1` must hold
/// `cost * range * range` FE, so a plane is only started when it can be paid
/// for (upstream then still charges per block).
pub fn reserve(upgrades: &[UpgradeEntry], range: i32) -> i32 {
    let side = range.clamp(1, MAX_RANGE);
    cost_per_block(upgrades).saturating_mul(side.saturating_mul(side))
}

/// The largest mining range the installed upgrades allow: 1 without a size
/// upgrade, else `2 * tier + 1` (3, 5, 7).
pub fn max_range(upgrades: &[UpgradeEntry]) -> i32 {
    let tier = tier(upgrades, "size_1");
    if tier <= 0 {
        1
    } else {
        (2 * tier + 1).clamp(1, MAX_RANGE)
    }
}

/// A range as the gadget may hold it: odd, at least 1, at most [`MAX_RANGE`].
pub fn clamp_range(range: i32) -> i32 {
    let range = range.clamp(1, MAX_RANGE);
    if range % 2 == 0 { range + 1 } else { range }
}

/// How many of `targets` blocks a buffer of `energy` FE pays for, in order;
/// a free gadget (`cost <= 0`) pays for all of them.
pub fn affordable(energy: i32, cost: i32, targets: usize) -> usize {
    if cost <= 0 {
        return targets;
    }
    let paid = energy.max(0) / cost;
    (paid.max(0) as usize).min(targets)
}

/// `MiningCollect.isValid`: air, a fluid that is not a waterlogged block, an
/// unbreakable block (hardness below 0), a block entity and a door are not
/// targets.
pub fn is_valid(candidate: &Candidate) -> bool {
    !candidate.air
        && (!candidate.fluid || candidate.waterlogged)
        && candidate.hardness >= 0.0
        && !candidate.block_entity
        && !candidate.door
}

/// `Direction.fromYRot`: the horizontal direction a yaw faces, as unit vectors
/// (`[0,0,1]` is south, `[1,0,0]` east).
pub fn facing_from_yaw(yaw: f64) -> Pos {
    // Vanilla's `VALUES` for `fromYRot`, one per 90 degrees from south.
    const FACINGS: [Pos; 4] = [[0, 0, 1], [-1, 0, 0], [0, 0, -1], [1, 0, 0]];
    let quarter = (yaw / 90.0 + 0.5).floor() as i32 & 3;
    FACINGS[quarter as usize]
}

/// `Direction.getClockWise` around the Y axis (north, east, south, west); any
/// other vector is returned unchanged.
pub fn clockwise(direction: Pos) -> Pos {
    match direction {
        [0, 0, -1] => [1, 0, 0],
        [1, 0, 0] => [0, 0, 1],
        [0, 0, 1] => [-1, 0, 0],
        [-1, 0, 0] => [0, 0, -1],
        other => other,
    }
}

/// `Direction.getCounterClockWise` around the Y axis.
pub fn counter_clockwise(direction: Pos) -> Pos {
    match direction {
        [0, 0, -1] => [-1, 0, 0],
        [-1, 0, 0] => [0, 0, 1],
        [0, 0, 1] => [1, 0, 0],
        [1, 0, 0] => [0, 0, -1],
        other => other,
    }
}

fn negate(direction: Pos) -> Pos {
    [-direction[0], -direction[1], -direction[2]]
}

fn offset(start: Pos, direction: Pos, amount: i32) -> Pos {
    [
        start[0] + direction[0] * amount,
        start[1] + direction[1] * amount,
        start[2] + direction[2] * amount,
    ]
}

/// `MiningCollect.collect`: the block to mine around `start`, on the face
/// `face` of it, for a gadget of `range` and `size_mode`. The result is every
/// position of the plane, before the caller drops the illegal ones; a `range`
/// of 1 is only `start`.
pub fn collect(
    start: Pos,
    face: Pos,
    player_facing: Pos,
    range: i32,
    size_mode: SizeMode,
    player_y: f64,
) -> Vec<Pos> {
    let range = clamp_range(range);
    if range == 1 {
        return vec![start];
    }
    // A face pointing up or down makes the plane horizontal; the player's
    // facing stands in for "up" there, as upstream does.
    let vertical = face[0] == 0 && face[2] == 0;
    let up = if vertical { player_facing } else { [0, 1, 0] };
    let down = negate(up);
    let right = if vertical {
        clockwise(up)
    } else {
        counter_clockwise(face)
    };
    let left = negate(right);

    let middle = (range - 1) / 2;
    let mut up_range = middle;
    let mut down_range = middle;
    if !vertical && range > 3 {
        let flatten = match size_mode {
            // Aimed at something near the feet: dig upward from the floor.
            SizeMode::Auto => (player_y - f64::from(start[1])).abs() < 2.0,
            SizeMode::Pathway => true,
            SizeMode::Normal => false,
        };
        if flatten {
            down_range = 1;
            up_range = range - 2;
        }
    }

    let top_left = offset(offset(start, up, up_range), left, middle);
    let bottom_right = offset(offset(start, down, down_range), right, middle);
    let mut positions = Vec::with_capacity((range * range) as usize);
    for x in min_max(top_left[0], bottom_right[0]) {
        for y in min_max(top_left[1], bottom_right[1]) {
            for z in min_max(top_left[2], bottom_right[2]) {
                positions.push([x, y, z]);
            }
        }
    }
    positions
}

fn min_max(a: i32, b: i32) -> std::ops::RangeInclusive<i32> {
    a.min(b)..=a.max(b)
}

/// `MiningGadget.getHardness`: the ticks one block takes — the aimed plane's
/// average `hardness * 30 / toolSpeed`, scaled by the range, at least 1.
/// `efficiency` is the active efficiency tier, `haste` the amplifier of Haste
/// (0 when absent, +1 inside), `fatigue` whether Mining Fatigue is on.
pub fn break_ticks(
    hardness: &[f32],
    range: i32,
    efficiency: i32,
    haste: i32,
    fatigue: bool,
) -> i32 {
    let mut tool_speed = TOOL_SPEED;
    if efficiency > 0 {
        let tier = efficiency as f32;
        tool_speed += tier * tier + 1.0;
    }
    if haste > 0 {
        let level = (haste + 1) as f32;
        tool_speed += tool_speed * (level * 20.0 / 100.0);
    }
    if fatigue {
        tool_speed /= 3.0;
    }
    let average = if hardness.is_empty() {
        0.0
    } else {
        let total: f32 = hardness.iter().map(|value| value * 30.0 / tool_speed).sum();
        total / hardness.len() as f32
    };
    let ticks = (average * range.clamp(1, MAX_RANGE) as f32).floor();
    (ticks as i32).max(1)
}

/// The damage a gadget stack shows on its bar: how much of the buffer is gone,
/// as the engine's bar reads a stack's damage out of its item's `max_damage`.
pub fn bar_damage(energy: i32) -> u32 {
    (MAX_ENERGY - energy.clamp(0, MAX_ENERGY)) as u32
}

/// The face of the block at `block` that the hit point `point` lies on: the
/// axis the point is furthest from the block's centre on, and its sign. A ray
/// that met nothing outside the centre (a bounding-box hit exactly on the
/// middle) reads as the top face.
pub fn face_of(point: [f64; 3], block: Pos) -> Pos {
    let mut best = (0.0_f64, 0);
    for axis in 0..3 {
        let delta = (point[axis] - (f64::from(block[axis]) + 0.5)).abs();
        if delta > best.0 {
            best = (delta, axis);
        }
    }
    if best.0 == 0.0 {
        return [0, 1, 0];
    }
    let mut face = [0; 3];
    face[best.1] = if point[best.1] > f64::from(block[best.1]) + 0.5 {
        1
    } else {
        -1
    };
    face
}

/// Whether `name` is one of the mod's upgrades (`Upgrade.valueOf`), so a typo
/// is refused instead of stored on a gadget.
pub fn known_upgrade(name: &str) -> bool {
    const FLAT: [&str; 6] = [
        "empty",
        "silk",
        "void_junk",
        "magnet",
        "freezing",
        "light_placer",
    ];
    if FLAT.contains(&name) {
        return true;
    }
    let (base, tier) = match name.rsplit_once('_') {
        Some(pair) => pair,
        None => return false,
    };
    let highest = match base {
        "size" | "fortune" | "range" => 3,
        "battery" => 4,
        "efficiency" => 5,
        _ => return false,
    };
    if base == "battery" && tier == "creative" {
        return true;
    }
    tier.parse::<u32>()
        .is_ok_and(|tier| (1..=highest).contains(&tier))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upgrades(names: &[&str]) -> Vec<UpgradeEntry> {
        names.iter().map(|name| UpgradeEntry::new(name)).collect()
    }

    #[test]
    fn a_gadget_with_no_upgrades_costs_the_base_price() {
        assert_eq!(cost_per_block(&[]), BASE_COST);
        assert_eq!(cost_per_block(&[]), 200);
    }

    #[test]
    fn an_active_upgrade_adds_its_cost_and_an_inactive_one_does_not() {
        assert_eq!(cost_per_block(&upgrades(&["silk"])), 300);
        assert_eq!(cost_per_block(&upgrades(&["fortune_2"])), 260);
        let mut off = upgrades(&["silk"]);
        off[0].active = false;
        assert_eq!(cost_per_block(&off), 200);
        assert_eq!(
            cost_per_block(&upgrades(&["efficiency_5", "fortune_3", "magnet"])),
            200 + 50 + 100 + 25
        );
    }

    #[test]
    fn an_unknown_upgrade_costs_nothing() {
        assert_eq!(upgrade_cost("bogus"), 0);
        assert_eq!(upgrade_cost("upgrade_empty"), 0);
        assert_eq!(cost_per_block(&upgrades(&["bogus"])), BASE_COST);
        // A size or a range upgrade is free per block; it widens the plane.
        assert_eq!(cost_per_block(&upgrades(&["size_3", "range_3"])), 200);
    }

    #[test]
    fn a_creative_battery_makes_every_block_free() {
        assert_eq!(cost_per_block(&upgrades(&["battery_creative"])), 0);
        assert_eq!(cost_per_block(&upgrades(&["battery_creative", "silk"])), 0);
    }

    #[test]
    fn the_reserve_scales_with_the_range() {
        assert_eq!(reserve(&[], 1), 200);
        assert_eq!(reserve(&[], 3), 200 * 9);
        assert_eq!(reserve(&upgrades(&["silk"]), 5), 300 * 25);
        assert_eq!(reserve(&upgrades(&["battery_creative"]), 7), 0);
    }

    #[test]
    fn the_range_comes_from_the_size_upgrade() {
        assert_eq!(max_range(&[]), 1);
        assert_eq!(max_range(&upgrades(&["size_1"])), 3);
        assert_eq!(max_range(&upgrades(&["size_2"])), 5);
        assert_eq!(max_range(&upgrades(&["size_3"])), 7);
        let mut off = upgrades(&["size_2"]);
        off[0].active = false;
        assert_eq!(max_range(&off), 1);
    }

    #[test]
    fn a_range_is_odd_and_bounded() {
        assert_eq!(clamp_range(0), 1);
        assert_eq!(clamp_range(1), 1);
        assert_eq!(clamp_range(2), 3);
        assert_eq!(clamp_range(4), 5);
        assert_eq!(clamp_range(99), MAX_RANGE);
    }

    #[test]
    fn at_most_the_blocks_the_buffer_pays_for() {
        assert_eq!(affordable(1000, 200, 1), 1);
        assert_eq!(affordable(1000, 200, 9), 5);
        assert_eq!(affordable(199, 200, 1), 0);
        assert_eq!(affordable(0, 0, 7), 7);
        assert_eq!(affordable(500, 0, 3), 3);
        assert_eq!(affordable(-40, 200, 3), 0);
    }

    #[test]
    fn air_fluid_and_unbreakable_blocks_are_not_targets() {
        let solid = Candidate {
            hardness: 1.5,
            ..Candidate::default()
        };
        assert!(is_valid(&solid));
        assert!(!is_valid(&Candidate { air: true, ..solid }));
        assert!(!is_valid(&Candidate {
            fluid: true,
            ..solid
        }));
        assert!(!is_valid(&Candidate {
            hardness: -1.0,
            ..solid
        }));
    }

    #[test]
    fn a_waterlogged_block_is_a_target() {
        assert!(is_valid(&Candidate {
            fluid: true,
            waterlogged: true,
            hardness: 2.0,
            ..Candidate::default()
        }));
    }

    #[test]
    fn a_block_entity_or_a_door_is_not_a_target() {
        let solid = Candidate {
            hardness: 2.5,
            ..Candidate::default()
        };
        assert!(!is_valid(&Candidate {
            block_entity: true,
            ..solid
        }));
        assert!(!is_valid(&Candidate {
            door: true,
            ..solid
        }));
    }

    #[test]
    fn yaw_maps_to_vanillas_facings() {
        assert_eq!(facing_from_yaw(0.0), [0, 0, 1]);
        assert_eq!(facing_from_yaw(90.0), [-1, 0, 0]);
        assert_eq!(facing_from_yaw(180.0), [0, 0, -1]);
        assert_eq!(facing_from_yaw(-90.0), [1, 0, 0]);
        assert_eq!(facing_from_yaw(270.0), [1, 0, 0]);
    }

    #[test]
    fn the_horizontal_compass_turns_one_way_only() {
        let north = [0, 0, -1];
        assert_eq!(clockwise(north), [1, 0, 0]);
        assert_eq!(clockwise([1, 0, 0]), [0, 0, 1]);
        assert_eq!(counter_clockwise(north), [-1, 0, 0]);
        assert_eq!(counter_clockwise([0, 0, 1]), [1, 0, 0]);
        // A vertical direction has no horizontal turn.
        assert_eq!(clockwise([0, 1, 0]), [0, 1, 0]);
    }

    #[test]
    fn range_one_collects_only_the_aimed_block() {
        let start = [4, 70, -3];
        assert_eq!(
            collect(start, [0, 1, 0], [0, 0, 1], 1, SizeMode::Auto, 70.0),
            vec![start]
        );
    }

    #[test]
    fn a_three_range_makes_the_aimed_face_a_square() {
        // Aimed at a block's top: the plane is the 3 x 3 around it, flat.
        let got = collect([0, 64, 0], [0, 1, 0], [0, 0, 1], 3, SizeMode::Normal, 70.0);
        assert_eq!(got.len(), 9);
        assert!(got.iter().all(|pos| pos[1] == 64));
        assert!(got.contains(&[0, 64, 0]));
        assert!(got.contains(&[1, 64, 1]));
        assert!(got.contains(&[-1, 64, -1]));
        // Aimed at a block's north face: the plane is vertical, in x and y.
        let got = collect([0, 64, 0], [0, 0, -1], [0, 0, 1], 3, SizeMode::Normal, 70.0);
        assert_eq!(got.len(), 9);
        assert!(got.iter().all(|pos| pos[2] == 0));
        assert!(got.contains(&[0, 65, 0]));
        assert!(got.contains(&[0, 63, 0]));
    }

    #[test]
    fn a_pathway_digs_upward_from_the_floor() {
        // A range-5 pathway on a floor block: one row under the aimed block
        // and three above it, 5 wide.
        let got = collect(
            [0, 64, 0],
            [0, 0, -1],
            [0, 0, 1],
            5,
            SizeMode::Pathway,
            65.0,
        );
        assert_eq!(got.len(), 25);
        assert!(got.iter().all(|pos| pos[1] >= 63 && pos[1] <= 67));
        assert!(got.contains(&[0, 63, 0]));
        assert!(got.contains(&[0, 67, 0]));
        assert!(!got.contains(&[0, 62, 0]));
    }

    #[test]
    fn auto_flattens_the_plane_underfoot() {
        // Standing at the aimed block's level: the plane goes up, like a pathway.
        let flat = collect([0, 64, 0], [0, 0, -1], [0, 0, 1], 5, SizeMode::Auto, 64.6);
        assert_eq!(flat.len(), 25);
        assert!(flat.iter().all(|pos| pos[1] >= 63));
        // Aimed well above the feet: the plane stays centred.
        let centred = collect([0, 70, 0], [0, 0, -1], [0, 0, 1], 5, SizeMode::Auto, 64.0);
        assert_eq!(centred.len(), 25);
        assert!(centred.contains(&[0, 68, 0]));
        assert!(centred.contains(&[0, 72, 0]));
        // A small range is never flattened.
        let small = collect([0, 70, 0], [0, 0, -1], [0, 0, 1], 3, SizeMode::Auto, 64.0);
        assert_eq!(small.len(), 9);
        assert!(small.contains(&[0, 69, 0]));
    }

    #[test]
    fn break_ticks_follow_the_hardness_and_the_tool_speed() {
        // Stone: 1.5 * 30 / 8 = 5.625, floored.
        assert_eq!(break_ticks(&[1.5], 1, 0, 0, false), 5);
        // Dirt (0.5) is quicker, obsidian (50) is slower.
        assert_eq!(break_ticks(&[0.5], 1, 0, 0, false), 1);
        assert_eq!(break_ticks(&[50.0], 1, 0, 0, false), 187);
        // A plane averages its blocks and scales by the range.
        assert_eq!(break_ticks(&[1.5, 1.5, 1.5, 1.5], 3, 0, 0, false), 16);
        assert_eq!(break_ticks(&[1.5, 3.0], 1, 0, 0, false), 8);
    }

    #[test]
    fn efficiency_and_haste_shorten_the_break_and_fatigue_lengthens_it() {
        // Efficiency 3: toolSpeed = 8 + (9 + 1) = 18 -> 1.5 * 30 / 18 = 2.5.
        assert_eq!(break_ticks(&[1.5], 1, 3, 0, false), 2);
        // Haste 1 (+1 amplifier): toolSpeed * 1.2.
        assert_eq!(break_ticks(&[1.5], 1, 0, 0, false), 5);
        assert_eq!(break_ticks(&[1.5], 1, 0, 1, false), 4);
        // Mining Fatigue divides the speed by 3.
        assert_eq!(break_ticks(&[1.5], 1, 0, 0, true), 16);
    }

    #[test]
    fn a_break_always_takes_at_least_one_tick() {
        assert_eq!(break_ticks(&[0.0], 1, 0, 0, false), 1);
        assert_eq!(break_ticks(&[], 1, 0, 0, false), 1);
    }

    #[test]
    fn the_bar_shows_the_charge_that_is_missing() {
        assert_eq!(bar_damage(MAX_ENERGY), 0);
        assert_eq!(bar_damage(0), MAX_ENERGY as u32);
        assert_eq!(bar_damage(200), MAX_ENERGY as u32 - 200);
        // A value outside the buffer never makes the bar negative.
        assert_eq!(bar_damage(-5), MAX_ENERGY as u32);
        assert_eq!(bar_damage(MAX_ENERGY + 5), 0);
    }

    #[test]
    fn an_upgrade_is_found_by_its_base_name_and_keeps_its_tier() {
        let installed = upgrades(&["fortune_2", "efficiency_4", "battery_creative"]);
        assert!(active(&installed, "fortune_1"));
        assert!(active(&installed, "efficiency_1"));
        assert!(!active(&installed, "silk"));
        assert_eq!(tier(&installed, "fortune_1"), 2);
        assert_eq!(tier(&installed, "efficiency_1"), 4);
        assert_eq!(tier(&installed, "battery_1"), 4);
        assert_eq!(tier(&installed, "size_1"), 0);
        assert_eq!(
            find(&installed, "efficiency_1").map(|entry| entry.name.as_str()),
            Some("efficiency_4")
        );
    }

    #[test]
    fn a_hit_point_reads_the_face_it_met() {
        let block = [5, 64, 3];
        assert_eq!(face_of([5.5, 64.9, 3.5], block), [0, 1, 0]);
        assert_eq!(face_of([5.5, 64.1, 3.5], block), [0, -1, 0]);
        assert_eq!(face_of([4.98, 64.5, 3.5], block), [-1, 0, 0]);
        assert_eq!(face_of([6.02, 64.5, 3.5], block), [1, 0, 0]);
        assert_eq!(face_of([5.5, 64.5, 2.98], block), [0, 0, -1]);
        assert_eq!(face_of([5.5, 64.5, 4.02], block), [0, 0, 1]);
        // The centre of the block is no face at all, so the top is used.
        assert_eq!(face_of([5.5, 64.5, 3.5], block), [0, 1, 0]);
    }

    #[test]
    fn only_the_jars_upgrade_names_are_known() {
        for name in [
            "empty",
            "silk",
            "void_junk",
            "magnet",
            "freezing",
            "light_placer",
            "size_1",
            "size_3",
            "fortune_1",
            "fortune_3",
            "battery_1",
            "battery_creative",
            "range_2",
            "efficiency_1",
            "efficiency_5",
        ] {
            assert!(known_upgrade(name), "{name} is an upgrade");
        }
        for name in ["bogus", "size_4", "fortune", "efficiency_6", "silk_1", ""] {
            assert!(!known_upgrade(name), "{name} is not an upgrade");
        }
    }
}
