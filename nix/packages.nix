{
  craneLib,
  pkgs,
  src,
}:
let
  lintelMeta = craneLib.crateNameFromCargoToml { cargoToml = ../crates/lintel/Cargo.toml; };

  commonArgs = {
    inherit src;
    inherit (lintelMeta) pname version;
    strictDeps = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # Helper: reads pname from a crate's Cargo.toml, sets cargoExtraArgs automatically
  mkPackage =
    cratePath: extraArgs:
    let
      meta = craneLib.crateNameFromCargoToml { cargoToml = "${cratePath}/Cargo.toml"; };
    in
    craneLib.buildPackage (
      commonArgs
      // {
        inherit cargoArtifacts;
        inherit (meta) pname;
        cargoExtraArgs = "-p ${meta.pname}";
      }
      // extraArgs
    );

  lintel = mkPackage ../crates/lintel {
    postInstall = ''
      installShellCompletion --cmd lintel \
        --bash <($out/bin/lintel completions bash) \
        --zsh <($out/bin/lintel completions zsh) \
        --fish <($out/bin/lintel completions fish)
    '';
    nativeBuildInputs = [ pkgs.installShellCompletion ];
  };

  lintel-schemastore-catalog = mkPackage ../crates/lintel-schemastore-catalog { };
in
{
  inherit lintel lintel-schemastore-catalog;
}
