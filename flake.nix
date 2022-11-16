{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    cargo2nix.url = "github:cargo2nix/cargo2nix";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, cargo2nix, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustVersion = "1.62.1";
        #
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            cargo2nix.overlays.default
            (import rust-overlay)
          ];
        };
        #
        rustPkgs = pkgs.rustBuilder.makePackageSet {
          inherit rustVersion;
          packageFun = import ./Cargo.nix;
        };
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo2nix.packages.${system}.cargo2nix
            rust-bin.stable.${rustVersion}.default
            #
            openssl
            pkg-config
          ];
        };

        packages = rec {
          sk = (rustPkgs.workspace.speki {}).bin;
          default = sk;
        };
      });
}
