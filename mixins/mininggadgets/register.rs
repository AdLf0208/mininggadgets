// Declare `/mininggadgets info|energy|range|upgrade`.
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::mininggadgets_state::on_register_commands(commands);
}
