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
        --bash <($out/bin/lintel --bpaf-complete-style-bash) \
        --zsh <($out/bin/lintel --bpaf-complete-style-zsh) \
        --fish <($out/bin/lintel --bpaf-complete-style-fish)
    '';
    nativeBuildInputs = [ pkgs.installShellFiles ];
  };

  lintel-schemastore-catalog = mkPackage ../crates/lintel-schemastore-catalog { };

  lintel-github-action = mkPackage ../crates/lintel-github-action { };

  cargo-furnish = mkPackage ../crates/cargo-furnish { };

  lintel-catalog-builder = mkPackage ../crates/lintel-catalog-builder { };

  npm-release-binaries = mkPackage ../crates/npm-release-binaries { };
in
{
  inherit
    lintel
    lintel-schemastore-catalog
    lintel-github-action
    cargo-furnish
    lintel-catalog-builder
    npm-release-binaries
    ;
}
