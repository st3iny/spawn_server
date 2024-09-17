{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell
{
  buildInputs = with pkgs;
  [
    # rust-specific
    openssl
    cargo
    clippy
    rustfmt

    # c-specific
    gcc
    gdb
  ];
}
