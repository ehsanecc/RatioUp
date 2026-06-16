{ config, lib, pkgs, ... }:

let
  cfg = config.services.ratioup;

  # Build a TOML attrset from the Nix options, dropping null/unset optionals.
  # The keys must match what src/config.rs expects.
  tomlSettings = lib.filterAttrs (_: v: v != null) {
    client = cfg.client;
    port = cfg.port;
    numwant = cfg.numwant;
    use_pid_file = cfg.usePidFile;
    min_upload_rate = cfg.minUploadRate;
    max_upload_rate = cfg.maxUploadRate;
    torrent_dir = toString cfg.torrentDir;
    output_stats =
      if cfg.outputStats != null then toString cfg.outputStats else null;
  };

  configFile = (pkgs.formats.toml { }).generate "ratioup-config.toml" tomlSettings;

  # Paths that need to be writable at runtime (beyond the StateDirectory).
  extraWritePaths =
    lib.optional
      (!(lib.hasPrefix "/var/lib/ratioup" (toString cfg.torrentDir)))
      (toString cfg.torrentDir)
    ++ lib.optional
      (cfg.outputStats != null
        && !(lib.hasPrefix "/var/lib/ratioup" (builtins.dirOf (toString cfg.outputStats))))
      (builtins.dirOf (toString cfg.outputStats));

in
{
  options.services.ratioup = {
    enable = lib.mkEnableOption "RatioUp torrent ratio faker";

    package = lib.mkOption {
      type = lib.types.package;
      description = lib.mdDoc "The RatioUp package to use.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "ratioup";
      description = lib.mdDoc "System user account that runs the service.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "ratioup";
      description = lib.mdDoc "System group that runs the service.";
    };

    client = lib.mkOption {
      type = lib.types.str;
      default = "Transmission_3_00";
      example = "qBittorrent_4_6_4";
      description = lib.mdDoc ''
        Torrent client identity to emulate. The full list is at
        <https://docs.rs/fake-torrent-client/latest/fake_torrent_client/clients/enum.ClientVersion.html>.
      '';
    };

    port = lib.mkOption {
      type = lib.types.nullOr lib.types.port;
      default = null;
      example = 55555;
      description = lib.mdDoc ''
        BitTorrent listen port advertised to trackers.
        `null` picks a random port in the range 49152–65534 each time the
        service starts.
      '';
    };

    numwant = lib.mkOption {
      type = lib.types.nullOr (lib.types.ints.between 1 65535);
      default = null;
      example = 8;
      description = lib.mdDoc ''
        Number of peers to request from the tracker.
        `null` uses the default for the emulated client.
      '';
    };

    usePidFile = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = lib.mdDoc ''
        Write a PID file to `$XDG_RUNTIME_DIR/ratio_up.pid`.
        Usually not needed under systemd.
      '';
    };

    minUploadRate = lib.mkOption {
      type = lib.types.ints.unsigned;
      default = 8192;
      example = 262144;
      description = lib.mdDoc ''
        Minimum fake upload rate in **bytes per second** per torrent
        (e.g. `8192` = 8 KB/s, `262144` = 256 KB/s).
      '';
    };

    maxUploadRate = lib.mkOption {
      type = lib.types.ints.unsigned;
      default = 2097152;
      example = 23068672;
      description = lib.mdDoc ''
        Maximum fake upload rate in **bytes per second** per torrent
        (e.g. `2097152` = 2 MB/s, `23068672` = 22 MB/s).
      '';
    };

    torrentDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/ratioup/torrents";
      description = lib.mdDoc ''
        Directory watched for `.torrent` files.
        Defaults to a subdirectory of the service state directory.
      '';
    };

    outputStats = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/var/www/html/ratio_up.json";
      description = lib.mdDoc ''
        If set, RatioUp writes a live JSON stats file to this path.
        Combine it with the `www/index.html` from the repository for a
        simple browser dashboard.
      '';
    };

    logLevel = lib.mkOption {
      type = lib.types.enum [ "error" "warn" "info" "debug" "trace" ];
      default = "info";
      description = lib.mdDoc "Log verbosity passed as the `RUST_LOG` environment variable.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users = lib.mkIf (cfg.user == "ratioup") {
      ratioup = {
        isSystemUser = true;
        group = cfg.group;
        description = "RatioUp service user";
        home = "/var/lib/ratioup";
      };
    };

    users.groups = lib.mkIf (cfg.group == "ratioup") {
      ratioup = { };
    };

    # Ensure the torrent directory (and optional stats parent dir) exist with
    # correct ownership before the service starts.
    systemd.tmpfiles.rules =
      [ "d ${cfg.torrentDir} 0750 ${cfg.user} ${cfg.group} - -" ]
      ++ lib.optional (cfg.outputStats != null)
        "d ${builtins.dirOf (toString cfg.outputStats)} 0755 ${cfg.user} ${cfg.group} - -";

    systemd.services.ratioup = {
      description = "RatioUp — torrent ratio faker";
      documentation = [ "https://codeberg.org/slundi/RatioUp" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        ExecStart = "${cfg.package}/bin/RatioUp --config ${configFile}";
        User = cfg.user;
        Group = cfg.group;

        # Persistent state lives in /var/lib/ratioup (created & owned by systemd).
        StateDirectory = "ratioup";
        StateDirectoryMode = "0750";

        # Any paths outside StateDirectory that need write access at runtime.
        ReadWritePaths = extraWritePaths;

        Environment = "RUST_LOG=${cfg.logLevel}";
        Restart = "on-failure";
        RestartSec = "5s";

        # Security hardening
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        RestrictRealtime = true;
        SystemCallFilter = [ "@system-service" ];
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
      };
    };
  };
}
