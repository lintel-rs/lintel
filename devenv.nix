{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  packages = [
    pkgs.git
    pkgs.secretspec
    pkgs.nixfmt
    pkgs.nodePackages.prettier
  ];

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
    nixfmt.enable = true;
    prettier.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
