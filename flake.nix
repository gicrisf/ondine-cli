{
  description = "Native command-line renderer for Ondine Mermaid diagrams";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      forAllSystems =
        function: nixpkgs.lib.genAttrs systems (system: function (import nixpkgs { inherit system; }));
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "ondine-cli";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "compiler-0.0.0" = "sha256-WYf4q7RjssC9HYxebZOBmKDrIfMTyb2U9yDedp5fQEM=";
            };
          };

          doInstallCheck = true;
          installCheckPhase = ''
            runHook preInstallCheck
            $out/bin/ondine --version
            printf 'flowchart LR\n  A --> B\n' | $out/bin/ondine | grep -q '<svg'
            runHook postInstallCheck
          '';

          meta = {
            description = "Native command-line renderer for Ondine Mermaid diagrams.";
            license = pkgs.lib.licenses.mit;
            mainProgram = "ondine";
            platforms = pkgs.lib.platforms.unix;
          };
        };
      });

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) default;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            clippy
            nixfmt
            rustc
            rustfmt
          ];
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
