{
  description = "Nix shell for developing sxt-node";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in let
        buildInputs = with pkgs; [
          openssl
          perl # secretly a dependency of openssl
        ];

        nativeBuildInputs = with pkgs; [
          (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
          protobuf
          pkg-config
          rustPlatform.bindgenHook
          # for compiling arrow-ipc-no-std
          flatbuffers
        ];
      in {
        devShells.default = with pkgs;
          (mkShell.override {stdenv = gcc13Stdenv;}) {
            buildInputs = buildInputs;

            nativeBuildInputs = nativeBuildInputs;
          };

        packages.dummy = pkgs.runCommandWith {
          name = "dummy";
          derivationArgs.buildInputs = buildInputs;
          derivationArgs.nativeBuildInputs = nativeBuildInputs;
        } "mkdir $out";
      }
    );
}
