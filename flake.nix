{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    systems.url = "github:nix-systems/x86_64-linux";
    nixpkgs.url = "nixpkgs/nixos-unstable"; # unstable Nixpkgs
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [fenix.overlays.default];
      };
    in {
      devShells = {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
          ];

          buildInputs = [
            (fenix.packages.${system}.beta.withComponents ["cargo" "rustc" "rustfmt" "clippy" "rust-src"])
          ];

          packages = [
            self.formatter.${system}
          ];
        };
      };
      formatter = pkgs.nixfmt;
    });
}
