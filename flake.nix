{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      treefmt-nix,
      pre-commit-hooks,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Treefmt configuration
        treefmtEval = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            nixfmt.enable = true;
            rustfmt.enable = true;
            taplo.enable = true; # TOML formatter
          };
        };

        # Pre-commit hooks
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.config.build.wrapper;
            };
            cargo-check = {
              enable = true;
              entry = "${rustToolchain}/bin/cargo check";
              files = "\\.rs$";
              pass_filenames = false;
            };
            cargo-test = {
              enable = true;
              entry = "${rustToolchain}/bin/cargo test";
              files = "\\.rs$";
              pass_filenames = false;
            };
          };
        };
      in
      let
        rs-web = rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [ openssl ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        };
      in
      {
        packages = {
          inherit rs-web;
          default = rs-web;
        };

        # Treefmt formatter
        formatter = treefmtEval.config.build.wrapper;

        # Check for CI
        checks = {
          pre-commit-check = pre-commit-check;
          formatting = treefmtEval.config.build.check self;
        };

        devShells.default = pkgs.mkShell {
          inherit (pre-commit-check) shellHook;
          nativeBuildInputs =
            with pkgs;
            [
              rustToolchain
              pkg-config
              cargo-watch
              # Formatters
              treefmtEval.config.build.wrapper
              nixfmt-rfc-style
              taplo
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];
          buildInputs = with pkgs; [ openssl ];
        };
      }
    );
}
