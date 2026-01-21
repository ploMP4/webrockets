{
  description = "webrockets development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            name = "webrockets";

            buildInputs = with pkgs; [
              rustc
              cargo
              python313
              uv
              gcc
              gnumake
              deno
              docker
              docker-compose
              pnpm
            ];

            shellHook = ''
              export PYO3_PYTHON=${pkgs.python313}/bin/python
            '';
          };
        });
    };
}
