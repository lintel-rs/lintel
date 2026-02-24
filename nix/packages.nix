{
  craneLib,
  craneLibStatic ? null,
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

  lintel-config-schema-generator = mkPackage ../crates/lintel-config-schema-generator { };

  lintel-github-action = mkPackage ../crates/lintel-github-action { };

  npm-release-binaries = mkPackage ../crates/npm-release-binaries { };

  packages = {
    inherit
      cargo-furnish
      lintel
      lintel-catalog-builder
      lintel-config-schema-generator
      lintel-github-action
      npm-release-binaries
      ;
  };

  # Static musl packages (Linux only)
  staticPackages = pkgs.lib.optionalAttrs (craneLibStatic != null) (
    let
      staticCargoArtifacts = craneLibStatic.buildDepsOnly commonArgs;

      mkStaticPackage =
        cratePath:
        let
          meta = craneLibStatic.crateNameFromCargoToml { cargoToml = "${cratePath}/Cargo.toml"; };
        in
        craneLibStatic.buildPackage (
          commonArgs
          // {
            cargoArtifacts = staticCargoArtifacts;
            inherit (meta) pname;
            cargoExtraArgs = "-p ${meta.pname}";
          }
        );
    in
    {
      lintel-static = mkStaticPackage ../crates/lintel;
      lintel-github-action-static = mkStaticPackage ../crates/lintel-github-action;
    }
  );
in
packages
// staticPackages
// {
  default = lintel;
  all = pkgs.symlinkJoin {
    name = "lintel-all";
    paths = builtins.attrValues packages;
  };
}
