use crate::Shell;

/// Generate shell completion script and print to stdout.
pub fn run(shell: Shell) {
    match shell {
        Shell::Bash => print!("{}", bash_completions()),
        Shell::Zsh => print!("{}", zsh_completions()),
        Shell::Fish => print!("{}", fish_completions()),
        Shell::PowerShell => print!("{}", powershell_completions()),
    }
}

fn bash_completions() -> &'static str {
    r#"_lintel() {
    local cur prev commands
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    commands="check ci init convert completions version"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
        return 0
    fi

    case "${COMP_WORDS[1]}" in
        check|ci)
            case "$prev" in
                --exclude|--cache-dir|--format)
                    if [ "$prev" = "--format" ]; then
                        COMPREPLY=( $(compgen -W "json json5 jsonc toml yaml" -- "$cur") )
                    fi
                    return 0
                    ;;
                *)
                    COMPREPLY=( $(compgen -W "--verbose --exclude --cache-dir --no-cache --no-catalog --format" -- "$cur") $(compgen -f -- "$cur") )
                    return 0
                    ;;
            esac
            ;;
        convert)
            case "$prev" in
                --to)
                    COMPREPLY=( $(compgen -W "json yaml toml" -- "$cur") )
                    return 0
                    ;;
                *)
                    COMPREPLY=( $(compgen -W "--to" -- "$cur") $(compgen -f -- "$cur") )
                    return 0
                    ;;
            esac
            ;;
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish powershell" -- "$cur") )
            return 0
            ;;
    esac
}
complete -F _lintel lintel
"#
}

fn zsh_completions() -> &'static str {
    r#"#compdef lintel

_lintel() {
    local -a commands
    commands=(
        'check:Validate files against their schemas'
        'ci:Validate files with CI-friendly output'
        'init:Create a lintel.toml configuration file'
        'convert:Convert between JSON, YAML, and TOML formats'
        'completions:Generate shell completions'
        'version:Print version information'
    )

    _arguments -C \
        '1:command:->command' \
        '*::arg:->args'

    case $state in
        command)
            _describe 'command' commands
            ;;
        args)
            case $words[1] in
                check|ci)
                    _arguments \
                        '-v[Print additional diagnostics]' \
                        '--verbose[Print additional diagnostics]' \
                        '*--exclude[Exclude files matching pattern]:pattern:' \
                        '--cache-dir[Custom cache directory]:directory:_directories' \
                        '--no-cache[Disable schema caching]' \
                        '--no-catalog[Disable SchemaStore catalog]' \
                        '--format[Force file format]:format:(json json5 jsonc toml yaml)' \
                        '*:file:_files'
                    ;;
                convert)
                    _arguments \
                        '--to[Output format]:format:(json yaml toml)' \
                        '1:file:_files'
                    ;;
                completions)
                    _arguments '1:shell:(bash zsh fish powershell)'
                    ;;
            esac
            ;;
    esac
}

_lintel "$@"
"#
}

fn fish_completions() -> &'static str {
    r#"# Fish completions for lintel
complete -c lintel -n "__fish_use_subcommand" -a check -d "Validate files against their schemas"
complete -c lintel -n "__fish_use_subcommand" -a ci -d "Validate files with CI-friendly output"
complete -c lintel -n "__fish_use_subcommand" -a init -d "Create a lintel.toml configuration file"
complete -c lintel -n "__fish_use_subcommand" -a convert -d "Convert between JSON, YAML, and TOML formats"
complete -c lintel -n "__fish_use_subcommand" -a completions -d "Generate shell completions"
complete -c lintel -n "__fish_use_subcommand" -a version -d "Print version information"

# check/ci options
complete -c lintel -n "__fish_seen_subcommand_from check ci" -s v -l verbose -d "Print additional diagnostics"
complete -c lintel -n "__fish_seen_subcommand_from check ci" -l exclude -r -d "Exclude files matching pattern"
complete -c lintel -n "__fish_seen_subcommand_from check ci" -l cache-dir -r -d "Custom cache directory"
complete -c lintel -n "__fish_seen_subcommand_from check ci" -l no-cache -d "Disable schema caching"
complete -c lintel -n "__fish_seen_subcommand_from check ci" -l no-catalog -d "Disable SchemaStore catalog"
complete -c lintel -n "__fish_seen_subcommand_from check ci" -l format -r -a "json json5 jsonc toml yaml" -d "Force file format"

# convert options
complete -c lintel -n "__fish_seen_subcommand_from convert" -l to -r -a "json yaml toml" -d "Output format"

# completions options
complete -c lintel -n "__fish_seen_subcommand_from completions" -a "bash zsh fish powershell" -d "Shell type"
"#
}

fn powershell_completions() -> &'static str {
    r#"Register-ArgumentCompleter -CommandName lintel -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $commands = @('check', 'ci', 'init', 'convert', 'completions', 'version')
    $tokens = $commandAst.ToString().Split(' ')

    if ($tokens.Count -le 2) {
        $commands | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
    }
}
"#
}
