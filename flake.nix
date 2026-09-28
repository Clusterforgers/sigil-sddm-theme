{
  description = "Sigil: an SDDM login screen built around the Sigillum Dei Aemeth";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        sigil-sddm-theme = pkgs.callPackage ./nix/package.nix { };
        default = sigil-sddm-theme;
      });

      # For working on the plugin: `nix develop`, then cmake in qml-plugin/.
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
          packages = [ pkgs.qt6.qtdeclarative ];
        };
      });

      # Uses the system's own nixpkgs, so the plugin is built against the Qt SDDM runs.
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.sigil-sddm;
          theme = pkgs.callPackage ./nix/package.nix { };
        in
        {
          options.programs.sigil-sddm.enable = lib.mkEnableOption "the Sigil SDDM theme";

          config = lib.mkIf cfg.enable {
            services.displayManager.sddm = {
              package = lib.mkDefault pkgs.kdePackages.sddm;
              theme = "sigil";
              # Puts the plugin on the greeter's QML import path.
              extraPackages = [ theme ];
            };
            # Links share/sddm/themes/sigil where SDDM looks for themes.
            environment.systemPackages = [ theme ];
          };
        };
    };
}
