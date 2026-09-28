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
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default.plugin ];
          packages = [ pkgs.qt6.qtdeclarative ];
        };
      });

      # Builds with the system's own nixpkgs, so the plugin matches the Qt that SDDM runs.
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.sigil-sddm;
          settings = lib.foldl' lib.recursiveUpdate cfg.settings [
            (lib.optionalAttrs (cfg.systemInfo != null) { login.system_info = cfg.systemInfo; })
            (lib.optionalAttrs cfg.debug { login.debug = true; })
          ];
          theme = pkgs.callPackage ./nix/package.nix { inherit settings; };
        in
        {
          options.programs.sigil-sddm = {
            enable = lib.mkEnableOption "the Sigil SDDM theme";

            systemInfo = lib.mkOption {
              type = lib.types.nullOr lib.types.bool;
              default = null;
              example = true;
              description = ''
                Show the panel of system, host, kernel, CPU, memory, uptime and battery down
                the left of the login screen. Null leaves it to the figure file's
                `login.system_info`, which is off.
              '';
            };

            debug = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
                Find out how the login screen is really drawn. Shows, in its bottom-left
                corner, the graphics backend and GPU the greeter got (in red if it is the CPU
                standing in for one), frames per second, the longest gap between frames and
                the GPU's time per frame. The same goes to the journal every two seconds,
                with Qt's own account of the device it chose:

                  journalctl -b | grep -E 'sigil:|qt.rhi|qt.scenegraph'
              '';
            };

            settings = lib.mkOption {
              type = (pkgs.formats.json { }).type;
              default = { };
              example = lib.literalExpression ''
                {
                  effects.lightning.enabled = false;
                  effects.surge.charge = 1.5;
                }
              '';
              description = ''
                Settings laid over the theme's figure.json5, by the same names: objects merge
                key by key, anything else is replaced. See docs/figure.md for what there is.
                Changing these rebuilds only the theme's files, not the engine.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            services.displayManager.sddm = {
              package = lib.mkDefault pkgs.kdePackages.sddm;
              theme = "sigil";
              # Puts the plugin on the greeter's QML import path.
              extraPackages = [ theme ];
              # Qt's account of the backend and device it chose, and GPU timestamps for the
              # readout's GPU time.
              settings.General.GreeterEnvironment = lib.mkIf cfg.debug "QSG_INFO=1,QSG_RHI_PROFILE=1";
            };
            # Links share/sddm/themes/sigil where SDDM looks for themes.
            environment.systemPackages = [ theme ];
          };
        };
    };
}
