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

        # Helper to create cargo wrapper scripts
        mkCargoWrapper =
          name: cmd:
          pkgs.writeShellScript name ''
            export PATH="${rustToolchain}/bin:$PATH"
            ${cmd}
          '';

        # Pre-commit hooks for CI (no network access - only formatting checks)
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.config.build.wrapper;
            };
            cargo-fmt = {
              enable = true;
              entry = "${mkCargoWrapper "cargo-fmt-check" "cargo fmt --check"}";
              files = "\\.rs$";
              pass_filenames = false;
            };
          };
        };

        # Pre-commit hooks for local development (with network access)
        pre-commit-local = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.config.build.wrapper;
            };
            cargo-fmt = {
              enable = true;
              entry = "${mkCargoWrapper "cargo-fmt" "cargo fmt"}";
              files = "\\.rs$";
              pass_filenames = false;
            };
            cargo-check = {
              enable = true;
              entry = "${mkCargoWrapper "cargo-check" "cargo check"}";
              files = "(\\.rs$|Cargo\\.toml$|Cargo\\.lock$)";
              pass_filenames = false;
            };
            clippy = {
              enable = true;
              entry = "${mkCargoWrapper "cargo-clippy" "cargo clippy -- -D warnings"}";
              files = "(\\.rs$|Cargo\\.toml$|Cargo\\.lock$)";
              pass_filenames = false;
            };
            cargo-test = {
              enable = true;
              entry = "${mkCargoWrapper "cargo-test" "cargo test"}";
              files = "(\\.rs$|Cargo\\.toml$|Cargo\\.lock$)";
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
