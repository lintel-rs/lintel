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
      $out/bin/lintel man > lintel.1
      installManPage lintel.1
    '';
    nativeBuildInputs = [ pkgs.installShellFiles ];
  };

  lintel-schemastore-catalog = mkPackage ../crates/lintel-schemastore-catalog {
    postInstall = ''
      installShellCompletion --cmd lintel-schemastore-catalog \
        --bash <($out/bin/lintel-schemastore-catalog --bpaf-complete-style-bash) \
        --zsh <($out/bin/lintel-schemastore-catalog --bpaf-complete-style-zsh) \
        --fish <($out/bin/lintel-schemastore-catalog --bpaf-complete-style-fish)
      $out/bin/lintel-schemastore-catalog man > lintel-schemastore-catalog.1
      installManPage lintel-schemastore-catalog.1
    '';
    nativeBuildInputs = [ pkgs.installShellFiles ];
  };

  lintel-github-action = mkPackage ../crates/lintel-github-action { };

  cargo-furnish = mkPackage ../crates/cargo-furnish {
    postInstall = ''
      installShellCompletion --cmd cargo-furnish \
        --bash <($out/bin/cargo-furnish --bpaf-complete-style-bash) \
        --zsh <($out/bin/cargo-furnish --bpaf-complete-style-zsh) \
        --fish <($out/bin/cargo-furnish --bpaf-complete-style-fish)
      $out/bin/cargo-furnish man > cargo-furnish.1
      installManPage cargo-furnish.1
    '';
    nativeBuildInputs = [ pkgs.installShellFiles ];
  };

  lintel-catalog-builder = mkPackage ../crates/lintel-catalog-builder {
    postInstall = ''
      installShellCompletion --cmd lintel-catalog-builder \
        --bash <($out/bin/lintel-catalog-builder --bpaf-complete-style-bash) \
        --zsh <($out/bin/lintel-catalog-builder --bpaf-complete-style-zsh) \
        --fish <($out/bin/lintel-catalog-builder --bpaf-complete-style-fish)
      $out/bin/lintel-catalog-builder man > lintel-catalog-builder.1
      installManPage lintel-catalog-builder.1
    '';
    nativeBuildInputs = [ pkgs.installShellFiles ];
  };
in
{
  inherit
    lintel
    lintel-schemastore-catalog
    lintel-github-action
    cargo-furnish
    lintel-catalog-builder
    ;
}
