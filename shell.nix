{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell
{
  buildInputs = with pkgs;
  [
    openssl
    rustup
    pkg-config

    gcc
    gdb
  ];
}
