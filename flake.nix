{
  description = "stillo - AI-native terminal browser";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            pkg-config
            openssl
          ];

          shellHook = ''
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
          '';
        };
      }
    );
}
