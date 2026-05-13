#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
crate_dir=$script_dir
install_root=
debug_flag=
force_flag=
lock_flag=--locked
completions_dir=
manpage_dir=
setup_completion_shell=

show_help() {
    printf '%s\n' 'Usage: ./install.sh [--root DIR] [--debug] [--force] [--no-lock] [--check]'
    printf '%s\n' '                    [--install-completions DIR] [--install-manpage DIR]'
    printf '%s\n' '                    [--setup-completion[=SHELL]]'
    printf '%s\n' ''
    printf '%s\n' 'Options:'
    printf '%s\n' '  --root DIR              Install into a custom Cargo root directory'
    printf '%s\n' '  --debug                 Install the debug build'
    printf '%s\n' '  --force                 Reinstall even if already present'
    printf '%s\n' '  --no-lock               Skip Cargo lockfile enforcement'
    printf '%s\n' '  --check                 Only verify prerequisites and exit'
    printf '%s\n' '  --install-completions DIR  Write all shell completion scripts into DIR'
    printf '%s\n' '  --install-manpage DIR      Write the manpage into DIR'
    printf '%s\n' '  --setup-completion[=SHELL]'
    printf '%s\n' '                          Install the completion script into the user dir'
    printf '%s\n' '                          for SHELL (bash, zsh, fish). With no value,'
    printf '%s\n' '                          auto-detects from $SHELL.'
    printf '%s\n' '  -h, --help              Show this help'
    printf '%s\n' '  --version               Print the installer version'
    printf '%s\n' ''
    printf '%s\n' 'Completion:'
    printf '%s\n' '  bash/zsh/fish completions include a dynamic wrapper that reads'
    printf '%s\n' '  command/profile/workspace names from .btr.toml. PowerShell and'
    printf '%s\n' '  Elvish receive static completion. btr must be on PATH at'
    printf '%s\n' '  completion time for dynamic candidates to appear.'
}

warn_path() {
    cargo_home=${CARGO_HOME:-$HOME/.cargo}
    cargo_bin_dir=$cargo_home/bin

    if [ ! -d "$cargo_bin_dir" ]; then
        return 0
    fi

    found=0
    old_ifs=$IFS
    IFS=:
    for path_dir in ${PATH:-}; do
        if [ "$path_dir" = "$cargo_bin_dir" ]; then
            found=1
            break
        fi
    done
    IFS=$old_ifs

    if [ "$found" -eq 0 ]; then
        printf '%s\n' "warning: $cargo_bin_dir is not on PATH" >&2
    fi
}

detect_shell() {
    case "${SHELL:-}" in
        */bash) printf '%s' 'bash' ;;
        */zsh)  printf '%s' 'zsh' ;;
        */fish) printf '%s' 'fish' ;;
        *)      return 1 ;;
    esac
}

setup_completion() {
    shell=$1

    if [ "$shell" = "auto" ]; then
        if ! shell=$(detect_shell); then
            printf '%s\n' "error: could not auto-detect shell from \$SHELL (\"${SHELL:-}\")" >&2
            printf '%s\n' "       pass --setup-completion=bash|zsh|fish explicitly" >&2
            return 1
        fi
        printf '%s\n' "detected shell: $shell"
    fi

    case "$shell" in
        bash)
            target_dir=${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions
            target_file=$target_dir/btr
            clap_shell=bash
            ;;
        zsh)
            target_dir=${ZDOTDIR:-$HOME}/.zsh/completions
            target_file=$target_dir/_btr
            clap_shell=zsh
            ;;
        fish)
            target_dir=${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions
            target_file=$target_dir/btr.fish
            clap_shell=fish
            ;;
        powershell|pwsh|power-shell)
            printf '%s\n' "PowerShell auto-setup is not supported (profile paths vary)." >&2
            printf '%s\n' "  Run:  btr completions power-shell > \$PROFILE.CurrentUserAllHosts" >&2
            return 1
            ;;
        elvish)
            printf '%s\n' "Elvish auto-setup is not supported." >&2
            printf '%s\n' "  Run:  btr completions elvish > ~/.config/elvish/lib/btr-completion.elv" >&2
            printf '%s\n' "  Then in rc.elv add:  use btr-completion" >&2
            return 1
            ;;
        *)
            printf '%s\n' "error: unsupported shell \"$shell\"; supported: bash, zsh, fish" >&2
            return 1
            ;;
    esac

    mkdir -p "$target_dir"
    cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- completions "$clap_shell" > "$target_file"
    printf '%s\n' "installed $shell completion: $target_file"

    case "$shell" in
        bash)
            printf '%s\n' "  Ensure bash-completion is installed and sourced from ~/.bashrc:"
            printf '%s\n' "    [ -r /usr/share/bash-completion/bash_completion ] && . /usr/share/bash-completion/bash_completion"
            printf '%s\n' "  Then open a new bash session (or: exec bash)."
            ;;
        zsh)
            printf '%s\n' "  Add to ~/.zshrc BEFORE compinit:"
            printf '%s\n' "    fpath=($target_dir \$fpath)"
            printf '%s\n' "    autoload -U compinit && compinit"
            printf '%s\n' "  Then open a new zsh session (or: exec zsh)."
            ;;
        fish)
            printf '%s\n' "  Fish will pick it up automatically in the next session."
            ;;
    esac
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --root)
            shift
            if [ "$#" -eq 0 ]; then
                printf '%s\n' 'error: --root requires a directory' >&2
                exit 1
            fi
            install_root=$1
            shift
            continue
            ;;
        --debug)
            debug_flag=--debug
            shift
            continue
            ;;
        --force)
            force_flag=--force
            shift
            continue
            ;;
        --no-lock)
            lock_flag=
            shift
            continue
            ;;
        --check)
            if ! command -v cargo >/dev/null 2>&1; then
                printf '%s\n' 'error: cargo is required to install btr' >&2
                exit 1
            fi
            warn_path
            exit 0
            continue
            ;;
        --install-completions)
            shift
            if [ "$#" -eq 0 ]; then
                printf '%s\n' 'error: --install-completions requires a directory' >&2
                exit 1
            fi
            completions_dir=$1
            shift
            continue
            ;;
        --install-manpage)
            shift
            if [ "$#" -eq 0 ]; then
                printf '%s\n' 'error: --install-manpage requires a directory' >&2
                exit 1
            fi
            manpage_dir=$1
            shift
            continue
            ;;
        --setup-completion)
            setup_completion_shell=auto
            shift
            continue
            ;;
        --setup-completion=*)
            setup_completion_shell=${1#--setup-completion=}
            if [ -z "$setup_completion_shell" ]; then
                setup_completion_shell=auto
            fi
            shift
            continue
            ;;
        -h|--help)
            show_help
            exit 0
            continue
            ;;
        --version)
            printf '%s\n' 'btr install script 1.0.0'
            exit 0
            continue
            ;;
        *)
            printf '%s\n' "error: unknown option: $1" >&2
            show_help >&2
            exit 1
            continue
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'error: cargo is required to install btr' >&2
    exit 1
fi

warn_path

set -- cargo install --path "$crate_dir"
if [ -n "$lock_flag" ]; then
    set -- "$@" "$lock_flag"
fi
if [ -n "$debug_flag" ]; then
    set -- "$@" "$debug_flag"
fi
if [ -n "$force_flag" ]; then
    set -- "$@" "$force_flag"
fi
if [ -n "$install_root" ]; then
    set -- "$@" --root "$install_root"
fi

"$@"

if [ -n "$completions_dir" ]; then
    mkdir -p "$completions_dir"
    for shell in bash elvish fish power-shell zsh; do
        cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- completions "$shell" > "$completions_dir/btr.$shell"
    done
fi

if [ -n "$manpage_dir" ]; then
    mkdir -p "$manpage_dir"
    cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- manpage > "$manpage_dir/btr.1"
fi

if [ -n "$setup_completion_shell" ]; then
    setup_completion "$setup_completion_shell"
fi
