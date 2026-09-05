{
  description = "Anima: standalone terminal animations from Yazelix";

  inputs = {
    kinestra.url = "github:Yazelix/kinestra";
    recording-mars.url = "github:Yazelix/mars/21109e3ebc24b63da11bae644dfb9bab28ce0e18";
    recording-mars.inputs.nixpkgs.url = "github:NixOS/nixpkgs/567a49d1913ce81ac6e9582e3553dd90a955875f";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    asciiquarium = {
      url = "github:cablehead/asciiquarium-rs/beef5b7dae179937c67f9e1557e02d64be55fa71";
      flake = false;
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      asciiquarium,
      fenix,
      kinestra,
      recording-mars,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      animaPackage =
        system: pkgs:
        let
          # Match https://index.crates.io/config.json without defining a second Cargo source.
          # Remove when pinned nixpkgs uses this download base itself.
          importCargoLock = pkgs.rustPlatform.importCargoLock.override {
            fetchurl =
              args:
              pkgs.fetchurl (
                args
                // {
                  url =
                    builtins.replaceStrings
                      [ "https://crates.io/api/v1/crates/" ]
                      [ "https://static.crates.io/crates/" ]
                      args.url;
                }
              );
          };
          aquarium = pkgs.rustPlatform.buildRustPackage {
            pname = "asciiquarium-rs";
            version = "0.1.1-dev";
            src = asciiquarium;
            cargoDeps = importCargoLock { lockFile = "${asciiquarium}/Cargo.lock"; };

            meta = {
              description = "Aquarium animation in ASCII art";
              homepage = "https://github.com/cablehead/asciiquarium-rs";
              license = pkgs.lib.licenses.gpl2Plus;
              mainProgram = "asciiquarium-rs";
            };
          };
          rustToolchain = fenix.packages.${system}.combine [
            fenix.packages.${system}.stable.cargo
            fenix.packages.${system}.stable.rustc
          ];
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
          source = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.lock
              ./Cargo.toml
              ./LICENSE
              ./README.md
              ./src
            ];
          };
        in
        rustPlatform.buildRustPackage {
          pname = "anima";
          version = "0.2.0";

          src = source;
          cargoDeps = importCargoLock { lockFile = ./Cargo.lock; };
          cargoBuildFlags = [
            "--bin"
            "anima"
          ];
          YZS_ASCIQUARIUM_BIN = "${aquarium}/bin/asciiquarium-rs";

          doCheck = false;

          meta = {
            description = "Anima: standalone terminal animations from Yazelix";
            homepage = "https://github.com/Yazelix/anima";
            license = pkgs.lib.licenses.asl20;
            mainProgram = "anima";
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          anima = animaPackage system pkgs;
        in
        {
          default = anima;
          inherit anima;
        }
      );

      apps = forAllSystems (
        system:
        let
          anima = {
            type = "app";
            program = "${self.packages.${system}.anima}/bin/anima";
          };
        in
        {
          default = anima;
          inherit anima;
        }
        // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          record-demo = {
            type = "app";
            program = "${
              kinestra.lib.${system}.mkRecorder {
                name = "record-anima-demo";
                recipe = ./demo/record.rs;
                environment = {
                  ANIMA_BIN = "${self.packages.${system}.anima}/bin/anima";
                  ANIMA_MARS = recording-mars.packages.${system}.mars;
                };
              }
            }/bin/record-anima-demo";
          };
        }
      );

      checks = forAllSystems (system: {
        anima = self.packages.${system}.anima;
      });
    };
}
