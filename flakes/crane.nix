{ inputs, ... }:
{
  perSystem =
    { pkgs, ... }:
    let
      craneLib = inputs.crane.mkLib pkgs;
      src = craneLib.cleanCargoSource ./..;

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs =
          # [
          #   # Add additional build inputs here
          # ]
          # ++
          pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

        # Additional environment variables can be set directly
        # MY_CUSTOM_VAR = "some value";
      };

      # Build *just* the cargo dependencies, so we can reuse
      # all of that work (e.g. via cachix) when running in CI
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      # Build the actual crate itself, reusing the dependency
      # artifacts from above.
      my-crate = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;

          nativeBuildInputs = [ pkgs.makeWrapper ];

          # Bundle the runtime CLIs the tool spawns, so `nix run` works
          # without them on the caller's PATH. `nix` itself is intentionally
          # left out (see README requirements) — the `nix run` caller already has it.
          postInstall = ''
            wrapProgram $out/bin/quick-start \
              --prefix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.gh
                  pkgs.ghq
                  pkgs.pass
                  pkgs.git
                ]
              }
          '';
        }
      );
    in
    {
      _module.args = {
        inherit
          craneLib
          src
          commonArgs
          cargoArtifacts
          my-crate
          ;
      };
    };
}
