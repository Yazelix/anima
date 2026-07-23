{
  description = "Standalone terminal screen animations from Yazelix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    asciiquarium = {
      url = "github:luccahuguet/asciiquarium-rs/c78b76e84cd2c8b0e2f3b4e817e9cb90aee768a2";
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
      mkPkgs = system: nixpkgs.legacyPackages.${system};
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
          source = pkgs.lib.cleanSourceWith {
            name = "yzs-source";
            src = ./.;
            filter =
              path: _type:
              let
                relativePath = pkgs.lib.removePrefix ((toString ./.) + "/") (toString path);
              in
              relativePath != "target"
              && !pkgs.lib.hasPrefix "target/" relativePath
              && relativePath != ".git"
              && !pkgs.lib.hasPrefix ".git/" relativePath;
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
            description = "Standalone terminal screen animations from Yazelix";
            homepage = "https://github.com/luccahuguet/yazelix-screen";
            license = pkgs.lib.licenses.asl20;
            mainProgram = "yzs";
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = mkPkgs system;
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
