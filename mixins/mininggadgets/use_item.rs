// Right click in the air: ray-cast to the gadget's beam range, then fire.
fn observe_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::mininggadgets_state::on_use_item(game, stack, claimed);
}
