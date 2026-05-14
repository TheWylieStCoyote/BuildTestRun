use crate::{Cli, cli, constants};
use clap::CommandFactory;
use std::path::PathBuf;

pub const DYNAMIC_SENTINEL_START: &str = "# >>> btr dynamic completion >>>";
pub const DYNAMIC_SENTINEL_END: &str = "# <<< btr dynamic completion <<<";

pub fn shell_label(shell: cli::CompletionShell) -> &'static str {
    match shell {
        cli::CompletionShell::Bash => "bash",
        cli::CompletionShell::Zsh => "zsh",
        cli::CompletionShell::Fish => "fish",
        cli::CompletionShell::PowerShell => "power-shell",
        cli::CompletionShell::Elvish => "elvish",
    }
}

pub fn detect_shell_from_env<F>(get: F) -> Option<cli::CompletionShell>
where
    F: Fn(&str) -> Option<String>,
{
    let raw = get_nonempty(&get, "SHELL")?;
    let leaf = std::path::Path::new(&raw).file_name()?.to_str()?;
    match leaf {
        "bash" => Some(cli::CompletionShell::Bash),
        "zsh" => Some(cli::CompletionShell::Zsh),
        "fish" => Some(cli::CompletionShell::Fish),
        _ => None,
    }
}

pub fn default_install_path<F>(shell: cli::CompletionShell, get: F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    let home = PathBuf::from(get_nonempty(&get, "HOME")?);
    let base = match shell {
        cli::CompletionShell::Bash => {
            let data_home = get_nonempty(&get, "XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share"));
            data_home
                .join("bash-completion")
                .join("completions")
                .join("btr")
        }
        cli::CompletionShell::Zsh => {
            let zdotdir = get_nonempty(&get, "ZDOTDIR")
                .map(PathBuf::from)
                .unwrap_or(home);
            zdotdir.join(".zsh").join("completions").join("_btr")
        }
        cli::CompletionShell::Fish => {
            let config_home = get_nonempty(&get, "XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            config_home
                .join("fish")
                .join("completions")
                .join("btr.fish")
        }
        cli::CompletionShell::PowerShell | cli::CompletionShell::Elvish => return None,
    };
    Some(base)
}

fn get_nonempty<F>(get: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    get(key).filter(|value| !value.is_empty())
}

pub fn render(shell: cli::CompletionShell) -> String {
    let mut buffer: Vec<u8> = Vec::new();
    let clap_shell = match shell {
        cli::CompletionShell::Bash => clap_complete::Shell::Bash,
        cli::CompletionShell::Elvish => clap_complete::Shell::Elvish,
        cli::CompletionShell::Fish => clap_complete::Shell::Fish,
        cli::CompletionShell::PowerShell => clap_complete::Shell::PowerShell,
        cli::CompletionShell::Zsh => clap_complete::Shell::Zsh,
    };
    let mut command = Cli::command();
    clap_complete::generate(
        clap_shell,
        &mut command,
        constants::BINARY_NAME,
        &mut buffer,
    );
    let mut script = String::from_utf8_lossy(&buffer).into_owned();

    match shell {
        cli::CompletionShell::Bash => script.push_str(BASH_DYNAMIC),
        cli::CompletionShell::Zsh => script.push_str(ZSH_DYNAMIC),
        cli::CompletionShell::Fish => script.push_str(FISH_DYNAMIC),
        cli::CompletionShell::PowerShell | cli::CompletionShell::Elvish => {}
    }
    script
}

const BASH_DYNAMIC: &str = r#"
# >>> btr dynamic completion >>>
_btr_dynamic() {
    _btr "$@"
    local cur prev slot=""
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    case "$prev" in
        exec|show|explain|parallel) slot="commands" ;;
        --profile) slot="profiles" ;;
        --name) slot="workspace-names" ;;
        --tag) slot="workspace-tags" ;;
    esac

    if [ -z "$slot" ]; then
        local word
        for word in "${COMP_WORDS[@]:0:COMP_CWORD}"; do
            if [ "$word" = "parallel" ]; then
                slot="commands"
                break
            fi
        done
    fi

    if [ -n "$slot" ]; then
        local candidates
        candidates="$(btr complete "$slot" --cwd "$PWD" 2>/dev/null)"
        if [ -n "$candidates" ]; then
            COMPREPLY=( $(compgen -W "${candidates}" -- "${cur}") )
        fi
    fi
}
complete -F _btr_dynamic -o bashdefault -o default btr
# <<< btr dynamic completion <<<
"#;

const ZSH_DYNAMIC: &str = r#"
# >>> btr dynamic completion >>>
_btr_dynamic() {
    _btr "$@"

    local prev slot=""
    local -a candidates
    prev="${words[CURRENT-1]}"

    case "$prev" in
        exec|show|explain|parallel) slot="commands" ;;
        --profile) slot="profiles" ;;
        --name) slot="workspace-names" ;;
        --tag) slot="workspace-tags" ;;
    esac

    if [[ -z "$slot" ]]; then
        local word
        for word in "${words[@]:0:CURRENT-1}"; do
            if [[ "$word" == "parallel" ]]; then
                slot="commands"
                break
            fi
        done
    fi

    if [[ -n "$slot" ]]; then
        candidates=( ${(f)"$(btr complete "$slot" --cwd "$PWD" 2>/dev/null)"} )
        if (( ${#candidates[@]} )); then
            compadd -a candidates
        fi
    fi
}
compdef _btr_dynamic btr
# <<< btr dynamic completion <<<
"#;

const FISH_DYNAMIC: &str = r#"
# >>> btr dynamic completion >>>
complete -c btr -n '__fish_seen_subcommand_from exec show explain' -f -a '(btr complete commands --cwd (pwd) 2>/dev/null)'
complete -c btr -n '__fish_seen_subcommand_from parallel' -f -a '(btr complete commands --cwd (pwd) 2>/dev/null)'
complete -c btr -l profile -f -a '(btr complete profiles --cwd (pwd) 2>/dev/null)'
complete -c btr -n '__fish_seen_subcommand_from workspace' -l name -f -a '(btr complete workspace-names --cwd (pwd) 2>/dev/null)'
complete -c btr -n '__fish_seen_subcommand_from workspace' -l tag -f -a '(btr complete workspace-tags --cwd (pwd) 2>/dev/null)'
# <<< btr dynamic completion <<<
"#;
