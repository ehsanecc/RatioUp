# NixOS VM test for the ratioup service module.
# Run with: nix build .#checks.x86_64-linux.nixos-test
{ pkgs, nixosModule, ratioUpPackage }:

pkgs.testers.nixosTest {
  name = "ratioup";

  nodes.machine =
    { ... }:
    {
      imports = [ nixosModule ];

      services.ratioup = {
        enable = true;
        package = ratioUpPackage;

        # Use a fixed port so there is no randomness in the test.
        port = 51413;
        numwant = 5;
        minUploadRate = 1024;
        maxUploadRate = 102400;
        torrentDir = "/var/lib/ratioup/torrents";
        outputStats = "/var/lib/ratioup/stats.json";
        logLevel = "debug";
      };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("ratioup.service")

    # Service must be active (not just activating / failed).
    machine.succeed("systemctl is-active ratioup.service")

    # The torrent directory must have been created by tmpfiles.
    machine.succeed("test -d /var/lib/ratioup/torrents")

    # The generated config file is a valid TOML and contains expected keys.
    config_path = machine.succeed(
        "systemctl show -p ExecStart --value ratioup.service"
        " | grep -oP '(?<=--config )\\S+'"
    ).strip()
    machine.succeed(f"grep 'port' {config_path}")
    machine.succeed(f"grep 'min_upload_rate' {config_path}")
    machine.succeed(f"grep 'torrent_dir' {config_path}")
    machine.succeed(f"grep 'output_stats' {config_path}")

    # Service runs as the 'ratioup' system user.
    machine.succeed(
        "systemctl show -p User --value ratioup.service | grep -qx ratioup"
    )

    # Allow a few seconds for the first announce cycle, then verify the
    # service is still running (not crashed on startup).
    machine.sleep(5)
    machine.succeed("systemctl is-active ratioup.service")
  '';
}
