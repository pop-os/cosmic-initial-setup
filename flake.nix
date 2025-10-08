{
  description = "COSMIC Initial setup";

  inputs = {
    flake-compat = {
      type = "github";
      owner = "edolstra";
      repo = "flake-compat";
    };

    flake-parts = {
      type = "github";
      owner = "hercules-ci";
      repo = "flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    nixpkgs = {
      type = "github";
      owner = "NixOS";
      repo = "nixpkgs";
      ref = "nixpkgs-unstable";
    };

    systems = {
      type = "github";
      owner = "nix-systems";
      repo = "default-linux";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;

      perSystem =
        { pkgs, self', ... }:
        let
          inherit (pkgs) lib;
        in
        {
          devShells.default = pkgs.mkShell {
            inputsFrom = [ self'.packages.cosmic-initial-setup ];

            packages = [
              pkgs.cargo
              pkgs.clippy
              pkgs.rustc
              pkgs.rustfmt
            ];
          };

          packages = {
            default = self'.packages.cosmic-initial-setup;

            cosmic-initial-setup = pkgs.cosmic-initial-setup.overrideAttrs {
              src = lib.fileset.toSource {
                root = ./.;
                fileset = lib.fileset.gitTracked ./.;
              };

              cargoBuildFeatures = [ "nixos" ];
              patches = [ ];
            };
          };
        };
    };
}
