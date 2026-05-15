use domm_game::first_playable_fixture;

const CANISTER_NAME: &str = "degens";
const CANISTER_PACKAGE: &str = "domm-degens-canister";

#[test]
fn pocket_ic_layer_has_stable_canister_target() {
    let fixture = first_playable_fixture();

    assert_eq!(CANISTER_NAME, "degens");
    assert_eq!(CANISTER_PACKAGE, "domm-degens-canister");
    assert_eq!(fixture.ids.session_id, "fixture-session-first-playable");
}
