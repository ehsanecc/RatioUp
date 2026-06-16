# Compatibility entry-point for users who consume the module without the flake.
# Usage:  imports = [ (import /path/to/RatioUp) ];
#         services.ratioup.package = ...;  # required — set to your own build
import ./nixos/modules/ratioup.nix
