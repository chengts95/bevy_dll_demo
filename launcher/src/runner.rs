use std::io::Write;

fn launch_runner(
    root: &Path,
    command_line: &str,
    save_path: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    let (program, program_args) = parse_runner_command(command_line)?;
    let program = resolve_program(root, &program);

    let log_path = root.join("launcher_run.log");
    let mut stdout = fs::File::create(&log_path)
        .map_err(|err| format!("failed to create {}: {err}", log_path.display()))?;
    writeln!(stdout, "LAUNCHER: starting `{command_line}`")?;
    writeln!(stdout, "LAUNCHER: start in `{}`", root.display())?;
    writeln!(stdout, "LAUNCHER: game file `{save_path}`")?;
    let stderr = stdout
        .try_clone()
        .map_err(|err| format!("failed to open runner log for errors: {err}"))?;

    let mut child = std::process::Command::new(&program)
        .args(&program_args)
        .current_dir(root)
        .env("BEVY_GAME_FILE", save_path)
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|err| format!("failed to start {}: {err}", program.display()))?;
    let pid = child.id();

    std::thread::spawn(move || {
        let message = match child.wait() {
            Ok(status) => format_exit_status(status),
            Err(err) => format!("LAUNCHER: failed to wait for process {pid}: {err}"),
        };
        if let Ok(mut log) = fs::OpenOptions::new().append(true).open(&log_path) {
            let _ = writeln!(log, "\n{message}");
        }
    });

    Ok(pid)
}

fn format_exit_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("LAUNCHER: process exited with code {code}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("LAUNCHER: process terminated by signal {signal}");
        }
    }

    "LAUNCHER: process terminated without an exit code".to_string()
}

fn parse_runner_command(
    command_line: &str,
) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let mut args = shell_words::split(command_line)
        .map_err(|err| format!("invalid runner_cmd: {err}"))?
        .into_iter();
    let program = args.next().ok_or("runner_cmd must not be empty")?;
    Ok((program, args.collect()))
}

fn resolve_program(root: &Path, program: &str) -> PathBuf {
    let path = Path::new(program);
    if path.is_absolute() {
        return path.to_path_buf();
    }

    if program.contains(std::path::MAIN_SEPARATOR) || program.contains('/') || program.contains('\\')
    {
        return root.join(path);
    }

    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{format_exit_status, parse_runner_command, resolve_program};
    use std::path::Path;

    #[test]
    fn parses_program_arguments_and_quotes() {
        let (program, args) = parse_runner_command("cargo run -p 'game runner'").unwrap();
        assert_eq!(program, "cargo");
        assert_eq!(args, ["run", "-p", "game runner"]);
    }

    #[test]
    fn rejects_an_empty_command() {
        assert!(parse_runner_command("  ").is_err());
    }

    #[test]
    fn resolves_relative_programs_against_start_dir() {
        assert_eq!(
            resolve_program(Path::new("/game"), "./game_runner"),
            Path::new("/game").join("./game_runner")
        );
    }

    #[test]
    fn leaves_path_lookup_commands_unqualified() {
        assert_eq!(resolve_program(Path::new("/game"), "cargo"), Path::new("cargo"));
    }

    #[cfg(unix)]
    #[test]
    fn reports_nonzero_exit_codes() {
        let status = std::process::Command::new("sh")
            .args(["-c", "exit 7"])
            .status()
            .unwrap();
        assert_eq!(format_exit_status(status), "LAUNCHER: process exited with code 7");
    }
}
