{ inputs, ... }:
{
  perSystem =
    {
      pkgs,
      craneLib,
      src,
      commonArgs,
      cargoArtifacts,
      my-crate,
      ...
    }:
    {
      checks = {
        # Build the crate as part of `nix flake check` for convenience
        inherit my-crate;

        # Run clippy (and deny all warnings) on the crate source,
        # again, reusing the dependency artifacts from above.
        #
        # Note that this is done as a separate derivation so that
        # we can block the CI if there are issues here, but not
        # prevent downstream consumers from building our crate by itself.
        my-crate-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          }
        );

        my-crate-doc = craneLib.cargoDoc (
          commonArgs
          // {
            inherit cargoArtifacts;
            # This can be commented out or tweaked as necessary, e.g. set to
            # `--deny rustdoc::broken-intra-doc-links` to only enforce that lint
            env.RUSTDOCFLAGS = "--deny warnings";
          }
        );

        # Check formatting
        my-crate-fmt = craneLib.cargoFmt {
          inherit src;
        };

        my-crate-toml-fmt = craneLib.taploFmt {
          src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          # taplo arguments can be further customized below as needed
          # taploExtraArgs = "--config ./taplo.toml";
        };

        # Audit dependencies
        my-crate-audit = craneLib.cargoAudit {
          inherit src;
          inherit (inputs) advisory-db;
        };

        # Audit licenses
        my-crate-deny = craneLib.cargoDeny {
          inherit src;
        };

        # Run tests with cargo-nextest
        # Consider setting `doCheck = false` on `my-crate` if you do not want
        # the tests to run twice
        my-crate-nextest = craneLib.cargoNextest (
          commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          }
        );
      };
    };
}
