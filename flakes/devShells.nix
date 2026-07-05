_: {
  perSystem =
    {
      config,
      self',
      craneLib,
      ...
    }:
    {
      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        inherit (self') checks;

        # Additional dev-shell environment variables can be set directly
        # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = config.pre-commit.settings.enabledPackages;

        shellHook = ''
          ${config.pre-commit.shellHook}
        '';
      };
    };
}
