{
  description = "Anima: standalone terminal animations from Yazelix";

  inputs = {
    kinestra.url = "github:Yazelix/kinestra";
    nova-rio = {
      url = "github:Yazelix/nova-rio/aead5c08d1d5b8396fedf1038b609144b93cbe4c";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      kinestra,
      nova-rio,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      fontPackage = pkgs: pkgs.runCommand "accelerando-terminal-font" { } ''
        install -Dm444 ${./fonts/accelerando-terminal.ttf} $out/share/fonts/truetype/accelerando-terminal.ttf
        install -Dm444 ${./fonts/accelerando-terminal-bold.ttf} $out/share/fonts/truetype/accelerando-terminal-bold.ttf
        install -Dm444 ${./fonts/NOTICE} $out/share/licenses/accelerando-terminal/NOTICE
      '';
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
              ./fonts/NOTICE
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
          doCheck = false;

          postInstall = ''
            install -Dm444 fonts/NOTICE $out/share/licenses/anima/accelerando-font.NOTICE
            install -Dm444 src/accelerando-glyphs.NOTICE $out/share/licenses/anima/accelerando-glyphs.NOTICE
            ln -s ${fontPackage pkgs}/share/fonts $out/share/fonts
          '';

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
          accelerando-font = fontPackage pkgs;
        }
      );

      apps = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
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
                  ANIMA_RIO = "${nova-rio.packages.${system}.nrio}/bin/nrio";
                  ANIMA_RIO_CONFIG = pkgs.replaceVars ./demo/rio/config.toml {
                    accelerandoDir = "${self.packages.${system}.accelerando-font}/share/fonts/truetype";
                    jetbrainsMonoDir = "${pkgs.jetbrains-mono}/share/fonts/truetype";
                    symbolsNerdDir = "${pkgs.nerd-fonts.symbols-only}/share/fonts/truetype/NerdFonts/Symbols";
                    notoSymbolsDir = "${pkgs.noto-fonts}/share/fonts/noto";
                    notoEmojiDir = "${pkgs.noto-fonts-color-emoji}/share/fonts/noto";
                  };
                  # Match Nova's Linux Rio wrapper so the private display finds Mesa's Vulkan drivers.
                  VK_ADD_DRIVER_FILES = "${pkgs.mesa}/share/vulkan/icd.d";
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
