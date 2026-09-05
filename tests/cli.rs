use std::process::Command;

#[test]
fn standalone_and_integrated_commands_report_their_own_names() {
    for override_name in [None, Some("yzx anima")] {
        let name = override_name.unwrap_or("anima");
        for (arg, success, expected) in [
            ("--help", true, format!("Usage:\n  {name} [STYLE]")),
            (
                "not-a-style",
                false,
                format!("unsupported screen style `not-a-style`. Try `{name} --help`\n"),
            ),
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_anima"));
            command.arg(arg).env_remove("YAZELIX_SCREEN_COMMAND_NAME");
            if let Some(name) = override_name {
                command.env("YAZELIX_SCREEN_COMMAND_NAME", name);
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(if success { 0 } else { 1 }));
            let stdout = String::from_utf8(output.stdout).unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            if success {
                assert!(stdout.contains(&expected), "{stdout}");
                assert!(stderr.is_empty(), "{stderr}");
            } else {
                assert_eq!(stderr, expected);
                assert!(stdout.is_empty(), "{stdout}");
            }
        }
    }
}
