# Sigil

An SDDM login theme built around the *Sigillum Dei Aemeth*: the figure turns and answers as
you type, the password is written in random occult symbols, and Enter sets off a surge.

## NixOS

Add the flake as an input:

```nix
inputs.sigil-sddm = {
  url = "github:Clusterforgers/sigil-sddm-theme";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then import the module and enable it:

```nix
{ inputs, ... }: {
  imports = [ inputs.sigil-sddm.nixosModules.default ];

  services.displayManager.sddm.enable = true;
  programs.sigil-sddm.enable = true;
}
```

The module selects the theme and uses the Qt 6 SDDM (`kdePackages.sddm`); the plugin is
built with your system's nixpkgs, so it matches the Qt SDDM runs. Optional settings:

```nix
programs.sigil-sddm = {
  systemInfo = true;                           # system panel down the left side
  debug = true;                                # GPU and frame rate in the corner
  settings.effects.lightning.enabled = false;  # anything from figure.json5
};
```

Every setting is described in [docs/figure.md](docs/figure.md).
