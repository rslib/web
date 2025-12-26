{
  nixConfig = {
    extra-substituters = [ "https://rslib.cachix.org" ];
    extra-trusted-public-keys = [
      "rslib.cachix.org-1:8OHneG2sLeTDlsZ4AZyNh8zx2zAwoiZUKVPnl21B+58="
    ];
  };

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

        # Treefmt configuration (rustfmt via cargo-fmt hook for version consistency)
        treefmtEval = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            nixfmt.enable = true;
            taplo.enable = true; # TOML formatter
          };
        };

        # Pre-commit hooks for nix flake check (no network access)
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.config.build.wrapper;
            };
            cargo-fmt = {
              enable = true;
              entry =
                let
                  wrapper = pkgs.writeShellScript "cargo-fmt-check" ''
                    export PATH="${rustToolchain}/bin:$PATH"
                    cargo fmt --check
                  '';
                in
                "${wrapper}";
              files = "\\.rs$";
              pass_filenames = false;
            };
          };
        };

        # Local-only pre-commit hooks (with network for cargo)
        pre-commit-local = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.config.build.wrapper;
            };
            cargo-fmt = {
              enable = true;
              entry = "${rustToolchain}/bin/cargo fmt";
              files = "\\.rs$";
              pass_filenames = false;
            };
            cargo-check = {
              enable = true;
              entry = "${rustToolchain}/bin/cargo check";
              files = "\\.rs$";
              pass_filenames = false;
            };
            clippy = {
              enable = true;
              entry = "${rustToolchain}/bin/cargo clippy -- -D warnings";
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
          inherit (pre-commit-local) shellHook;
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
