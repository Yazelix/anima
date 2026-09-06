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

#[cfg(unix)]
#[test]
fn closing_the_terminal_exits_timed_and_interactive_playback() {
    use std::{
        fs::File,
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
        process::{Child, Stdio},
        time::{Duration, Instant},
    };

    struct Reap(Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    for style in ["aquarium", "boids_predator", "static", "logo"] {
        for timed in [false, true] {
            for controlling in [false, true] {
                let (mut master, mut slave) = (-1, -1);
                let size = libc::winsize {
                    ws_row: 24,
                    ws_col: 80,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                // SAFETY: openpty writes two fresh descriptors into live integer slots.
                assert_eq!(
                    unsafe {
                        libc::openpty(
                            &mut master,
                            &mut slave,
                            std::ptr::null_mut(),
                            std::ptr::null(),
                            &size,
                        )
                    },
                    0
                );
                // SAFETY: these descriptors are owned uniquely after successful openpty.
                let (mut master, slave) =
                    unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
                // Do not let the child inherit a master that keeps its own terminal alive.
                for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
                    // SAFETY: fcntl only changes flags on these live, owned descriptors.
                    assert_eq!(
                        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) },
                        0
                    );
                }
                let binary = std::env::var_os("ANIMA_TEST_BIN")
                    .unwrap_or_else(|| env!("CARGO_BIN_EXE_anima").into());
                let mut command = Command::new(binary);
                command
                    .arg(style)
                    .env("TERM", "xterm-256color")
                    .stdin(Stdio::from(slave.try_clone().unwrap()))
                    .stdout(Stdio::from(slave.try_clone().unwrap()))
                    .stderr(Stdio::from(slave));
                if timed {
                    // Closure must exit before even a long welcome deadline.
                    command.args(["--duration-seconds", "60"]);
                }
                // SAFETY: only async-signal-safe syscalls run between fork and exec.
                unsafe {
                    command.pre_exec(move || {
                        if libc::setsid() == -1
                            || (controlling && libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1)
                        {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
                let mut child = Reap(command.spawn().unwrap());
                drop(command);
                let deadline = Instant::now() + Duration::from_secs(2);
                let mut output = Vec::new();
                while !output.windows(8).any(|bytes| bytes == b"\x1b[?2026l") {
                    assert!(Instant::now() < deadline, "{style}: no initial frame");
                    let mut fd = libc::pollfd {
                        fd: master.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: poll borrows one initialized descriptor for this call.
                    if unsafe { libc::poll(&mut fd, 1, 50) } > 0 {
                        let mut bytes = [0; 8192];
                        let count = master.read(&mut bytes).unwrap();
                        assert!(count > 0);
                        output.extend_from_slice(&bytes[..count]);
                    }
                }
                // Close during the input wait, both with and without a controlling-PTY SIGHUP.
                std::thread::sleep(Duration::from_millis(5));
                drop(master);
                let deadline = Instant::now() + Duration::from_secs(2);
                while child.0.try_wait().unwrap().is_none() {
                    assert!(
                        Instant::now() < deadline,
                        "{style} (timed={timed}, controlling={controlling}) survived terminal closure"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}
