use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

pub fn script(shell: Shell, exe: &Path) -> String {
    match shell {
        Shell::Zsh => {
            let exe = posix_quote(exe);
            posix_script(
                &exe,
                &format!("if (( $+functions[compdef] )); then eval \"$(COMPLETE=zsh {exe})\"; fi"),
            )
        }
        Shell::Bash => {
            let exe = posix_quote(exe);
            posix_script(&exe, &format!("eval \"$(COMPLETE=bash {exe})\""))
        }
        Shell::Fish => fish_script(&fish_quote(exe)),
    }
}

fn posix_script(exe: &str, completion: &str) -> String {
    format!(
        "w3() {{\n\
        \x20   if [ \"$1\" != \"cd\" ]; then\n\
        \x20       {exe} \"$@\"\n\
        \x20       return\n\
        \x20   fi\n\
        \x20   local target\n\
        \x20   target=\"$({exe} \"$@\")\" || return\n\
        \x20   if [ -d \"$target\" ]; then\n\
        \x20       builtin cd -- \"$target\"\n\
        \x20   else\n\
        \x20       printf '%s\\n' \"$target\"\n\
        \x20   fi\n\
        }}\n\
        {completion}\n"
    )
}

fn fish_script(exe: &str) -> String {
    format!(
        "function w3\n\
        \x20   if test \"$argv[1]\" != cd\n\
        \x20       {exe} $argv\n\
        \x20       return\n\
        \x20   end\n\
        \x20   set -l target ({exe} $argv)\n\
        \x20   or return\n\
        \x20   if test -d \"$target\"\n\
        \x20       cd \"$target\"\n\
        \x20   else\n\
        \x20       printf '%s\\n' $target\n\
        \x20   end\n\
        end\n\
        COMPLETE=fish {exe} | source\n"
    )
}

fn posix_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn fish_quote(path: &Path) -> String {
    format!(
        "'{}'",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const EXE: &str = "/opt/w3/bin/w3";

    #[test]
    fn every_shell_calls_the_binary_by_its_absolute_path() {
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            let script = script(shell, Path::new(EXE));
            assert!(script.contains("'/opt/w3/bin/w3'"), "{script}");
            assert!(!script.contains("command w3"), "{script}");
        }
    }

    #[test]
    fn the_scripts_end_in_the_completion_line_of_their_shell() {
        let zsh = script(Shell::Zsh, Path::new(EXE));
        assert!(
            zsh.trim_end().ends_with(
                "if (( $+functions[compdef] )); then eval \"$(COMPLETE=zsh '/opt/w3/bin/w3')\"; fi"
            ),
            "{zsh}"
        );
        let bash = script(Shell::Bash, Path::new(EXE));
        assert!(
            bash.trim_end()
                .ends_with("eval \"$(COMPLETE=bash '/opt/w3/bin/w3')\""),
            "{bash}"
        );
        let fish = script(Shell::Fish, Path::new(EXE));
        assert!(
            fish.trim_end()
                .ends_with("COMPLETE=fish '/opt/w3/bin/w3' | source"),
            "{fish}"
        );
    }

    #[test]
    fn a_quote_in_the_path_is_escaped_for_each_shell() {
        let exe = Path::new("/opt/it's/w3");
        assert!(
            script(Shell::Zsh, exe).contains("'/opt/it'\\''s/w3'"),
            "{}",
            script(Shell::Zsh, exe)
        );
        assert!(
            script(Shell::Fish, exe).contains("'/opt/it\\'s/w3'"),
            "{}",
            script(Shell::Fish, exe)
        );
    }

    #[test]
    fn shell_names_round_trip_through_clap() {
        use clap::ValueEnum;

        assert_eq!(Shell::from_str("zsh", true), Ok(Shell::Zsh));
        assert_eq!(Shell::from_str("bash", true), Ok(Shell::Bash));
        assert_eq!(Shell::from_str("fish", true), Ok(Shell::Fish));
        assert!(Shell::from_str("elvish", true).is_err());
    }
}
