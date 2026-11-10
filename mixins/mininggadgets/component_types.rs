// Before content loads: the component ids the gadget's own items carry.
fn observe_component_types(types: &mut Vec<String>) {
    crate::game::mininggadgets_state::on_component_types(types);
}
