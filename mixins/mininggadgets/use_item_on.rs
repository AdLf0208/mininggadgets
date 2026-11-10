// Right click on a block: the laser fires at the aimed face, out of the
// gadget's own buffer, and the click is claimed so no off-hand item is used.
fn observe_use_item_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::mininggadgets_state::on_use_item_on(game, stack, pos, face, claimed);
}
