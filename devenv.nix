{ pkgs, ... }:

{
  packages = with pkgs; [
    git
    secretspec
  ];

  languages.rust.enable = true;

  scripts.lintel.exec = ''
    cargo run -p lintel -- "$@"
  '';

  git-hooks.hooks = {
    clippy = {
      enable = true;
      settings.denyWarnings = true;
    };
    rustfmt.enable = true;
    nixfmt.enable = true;
    prettier.enable = true;
  };

  cachix.pull = [ "lintel" ];

}
