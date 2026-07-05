_: {
  perSystem =
    { my-crate, ... }:
    {
      apps.default = {
        type = "app";
        program = "${my-crate}/bin/quick-start";
      };
    };
}
