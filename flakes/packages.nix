_: {
  perSystem =
    { my-crate, ... }:
    {
      packages = {
        default = my-crate;
      };
    };
}
