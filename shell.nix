{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "django_wsrs";

  buildInputs = with pkgs; [
    rustc
    cargo
    python313
    uv
    gcc
    gnumake
    deno
  ];
}
