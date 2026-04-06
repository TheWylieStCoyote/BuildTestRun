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

show_help() {
    printf '%s\n' 'Usage: ./install.sh [--root DIR] [--debug] [--force] [--no-lock] [--check] [--install-completions DIR] [--install-manpage DIR]'
    printf '%s\n' ''
    printf '%s\n' 'Options:'
    printf '%s\n' '  --root DIR   Install into a custom Cargo root directory'
    printf '%s\n' '  --debug      Install the debug build'
    printf '%s\n' '  --force      Reinstall even if already present'
    printf '%s\n' '  --no-lock    Skip Cargo lockfile enforcement'
    printf '%s\n' '  --check      Only verify prerequisites and exit'
    printf '%s\n' '  --install-completions DIR  Write shell completions into DIR'
    printf '%s\n' '  --install-manpage DIR      Write the manpage into DIR'
    printf '%s\n' '  -h, --help   Show this help'
    printf '%s\n' '  --version    Print the installer version'
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
    for shell in bash elvish fish powershell zsh; do
        cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- completions "$shell" > "$completions_dir/btr.$shell"
    done
fi

if [ -n "$manpage_dir" ]; then
    mkdir -p "$manpage_dir"
    cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- manpage > "$manpage_dir/btr.1"
fi
