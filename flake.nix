{
  description = "Fast, multi-format JSON Schema linter for all your config files";

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

        packages' = import ./nix/packages.nix { inherit craneLib pkgs src; };
        inherit (packages')
          lintel
          lintel-schemastore-catalog
          lintel-github-action
          cargo-furnish
          ;
      in
      {
        checks = {
          inherit lintel lintel-github-action;
        };

        packages = {
          inherit
            lintel
            lintel-schemastore-catalog
            lintel-github-action
            cargo-furnish
            ;
          default = lintel;
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          docker = import ./nix/docker.nix { inherit pkgs lintel; };
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
