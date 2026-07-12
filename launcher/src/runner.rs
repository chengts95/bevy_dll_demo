fn launch_runner(
    root: &Path,
    command_line: &str,
    save_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (program, program_args) = parse_runner_command(command_line)?;

    let log_path = root.join("launcher_run.log");
    let stdout = fs::File::create(&log_path)
        .map_err(|err| format!("failed to create {}: {err}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|err| format!("failed to open runner log for errors: {err}"))?;

    std::process::Command::new(&program)
        .args(&program_args)
        .current_dir(root)
        .env("BEVY_GAME_FILE", save_path)
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|err| format!("failed to start {program}: {err}"))?;

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::parse_runner_command;

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
}
