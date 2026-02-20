{ pkgs, lintel }:
pkgs.dockerTools.buildLayeredImage {
  name = "ghcr.io/lintel-rs/lintel";
  tag = "latest";
  contents = [
    lintel
    pkgs.cacert
  ];
  config = {
    Entrypoint = [ "${lintel}/bin/lintel" ];
    Labels = {
      "org.opencontainers.image.source" = "https://github.com/lintel-rs/lintel";
      "org.opencontainers.image.description" =
        "Fast, multi-format JSON Schema linter for all your config files";
    };
  };
}
