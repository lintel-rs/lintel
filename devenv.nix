{ pkgs, ... }:

{
  packages = with pkgs; [
    git
  ];

  languages.rust.enable = true;

  scripts.lintel.exec = ''
    cargo run --release -p lintel -- "$@"
  '';

  scripts.lintel-debug.exec = ''
    cargo run -p lintel -- "$@"
  '';

  scripts.cargo-furnish.exec = ''
    cargo run --release -p cargo-furnish -- "$@"
  '';

  git-hooks.hooks = {
    clippy = {
      enable = true;
      settings = {
        allFeatures = true;
        denyWarnings = true;
        extraArgs = "--all-targets";
      };
    };
    rustfmt.enable = true;
    nixfmt.enable = true;
    prettier.enable = true;
  };

  cachix.pull = [ "lintel" ];

}
