use crate::settings::Settings;

#[test]
fn shortcuts_keep_the_keys_a_command_takes_and_refuse_what_no_keymap_holds() {
    let chosen: Settings =
        serde_json::from_str(r#"{"shortcuts":{"clear":["ctrl-alt-l"],"quit":[]}}"#)
            .expect("shortcuts read");
    chosen
        .validate()
        .expect("a free key and a command left without one");
    assert_eq!(chosen.shortcuts["clear"], ["ctrl-alt-l"]);
    for refused in [
        r#"{"shortcuts":{"fly":["ctrl-alt-l"]}}"#,
        r#"{"shortcuts":{"clear":["ctrl-"]}}"#,
        r#"{"shortcuts":{"clear":["ctrl-alt-l"],"find":["ctrl-alt-l"]}}"#,
    ] {
        let settings: Settings = serde_json::from_str(refused).expect("it parses");
        assert!(settings.validate().is_err(), "{refused}");
    }
}
