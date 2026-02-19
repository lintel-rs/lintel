{
  description = "Lintel - Validate JSON and YAML files against JSON Schema";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        src =
          let
            inherit (pkgs) lib;
            testdataFilter = path: _type: (lib.hasInfix "testdata" path);
          in
          lib.cleanSourceWith {
            src = ./.;
            filter = path: type: (craneLib.filterCargoSources path type) || (testdataFilter path type);
          };

        commonArgs = {
          inherit src;
          pname = "lintel";
          version = "0.0.1";
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        lintel = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        checks = {
          inherit lintel;
        };

        packages = {
          inherit lintel;
          default = lintel;
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          docker = pkgs.dockerTools.buildLayeredImage {
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
                "org.opencontainers.image.description" = "Validate JSON, YAML, and TOML files against JSON Schema";
              };
            };
          };
        };

        apps.default = flake-utils.lib.mkApp {
          drv = lintel;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [ ];
        };
      }
    );
}
