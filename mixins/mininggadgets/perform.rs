// Run a `/mininggadgets` command a player typed in chat.
fn observe_perform(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::mininggadgets_state::on_perform_command(game, player, command, ran);
}
