{
  description = "Anima: standalone terminal animations from Yazelix";

  inputs = {
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
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      yzsPackage =
        system: pkgs:
        let
          aquarium = pkgs.rustPlatform.buildRustPackage {
            pname = "asciiquarium-rs";
            version = "0.1.1-dev";
            src = asciiquarium;
            cargoLock.lockFile = "${asciiquarium}/Cargo.lock";

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
          pname = "yzs";
          version = "0.1.0";

          src = source;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "--bin"
            "yzs"
          ];
          YZS_ASCIQUARIUM_BIN = "${aquarium}/bin/asciiquarium-rs";

          doCheck = false;

          meta = {
            description = "Anima: standalone terminal animations from Yazelix";
            homepage = "https://github.com/Yazelix/anima";
            license = pkgs.lib.licenses.asl20;
            mainProgram = "yzs";
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          yzs = yzsPackage system pkgs;
        in
        {
          default = yzs;
          yzs = yzs;
          yazelix_screen = yzs;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.yzs}/bin/yzs";
        };
        yzs = {
          type = "app";
          program = "${self.packages.${system}.yzs}/bin/yzs";
        };
        yazelix_screen = {
          type = "app";
          program = "${self.packages.${system}.yzs}/bin/yzs";
        };
      });

      checks = forAllSystems (system: {
        yzs = self.packages.${system}.yzs;
      });
    };
}
