{
  description = "Fast, multi-format JSON Schema linter for all your config files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem =
        {
          self',
          pkgs,
          system,
          ...
        }:
        let
          craneLib = inputs.crane.mkLib pkgs;
          craneLibStatic =
            if pkgs.stdenv.isLinux then
              let
                muslPkgs =
                  {
                    "x86_64-linux" = pkgs.pkgsCross.musl64;
                    "aarch64-linux" = pkgs.pkgsCross.aarch64-multiplatform-musl;
                  }
                  .${system};
              in
              inputs.crane.mkLib muslPkgs
            else
              null;

          packages = import ./nix/packages.nix {
            inherit
              craneLib
              craneLibStatic
              pkgs
              ;
          };
        in
        {
          checks = {
            inherit (packages) lintel;
          };

          packages =
            packages
            // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
              docker = import ./nix/docker.nix {
                inherit pkgs;
                lintel = packages.lintel;
              };
            };

          devShells.default = craneLib.devShell {
            checks = self'.checks;
          };
        };
    };
}
