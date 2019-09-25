let
  pkgs = import ./nixpkgs.nix {};
in
pkgs.stdenv.mkDerivation {
  buildInputs = with pkgs; [
    bash
    cargo
    cfssl
    conmon
    cni-plugins
    cri-o
    etcd
    iptables
    kubernetes
    runc
    rustPackages.clippy
    utillinux
  ];

  LANG = "en_US.UTF-8";
  name = "shell";
}
