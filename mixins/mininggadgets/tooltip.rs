// The gadget's charge, its per-block price and its mining range.
fn observe_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(Text, u32)>,
) {
    crate::game::mininggadgets_state::on_tooltip(game, stack, lines);
}
