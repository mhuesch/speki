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
        rustVersion = "2022-11-01";
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
          rustChannel = "nightly";
          packageFun = import ./Cargo.nix;

          packageOverrides = pkgs: pkgs.rustBuilder.overrides.all ++ [

            (pkgs.rustBuilder.rustLib.makeOverride {
                name = "alsa-sys";
                overrideAttrs = drv: {
                  propagatedBuildInputs = drv.propagatedBuildInputs or [ ] ++ [
                    pkgs.alsaLib
                  ];
                };
            })
          ];
        };


      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo2nix.packages.${system}.cargo2nix
            rust-bin.nightly.${rustVersion}.default
            #
            alsaLib
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
