{ pkgs, lib, config, inputs, ... }:

{
  packages = [ pkgs.git pkgs.secretspec ];

  languages.rust.enable = true;

  scripts.lintel.exec = ''
    cargo run -p lintel -- "$@"
  '';

  # https://devenv.sh/git-hooks/
  git-hooks.hooks = {
    clippy = {
      enable = true;
      settings.denyWarnings = true;
    };
    rustfmt.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
