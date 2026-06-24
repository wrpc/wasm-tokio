{
  nixConfig.extra-substituters = [
    "https://bytecodealliance.cachix.org"
    "https://nixify.cachix.org"
    "https://crane.cachix.org"
    "https://nix-community.cachix.org"

  ];
  nixConfig.extra-trusted-substituters = [
    "https://bytecodealliance.cachix.org"
    "https://nixify.cachix.org"
    "https://crane.cachix.org"
    "https://nix-community.cachix.org"
  ];
  nixConfig.extra-trusted-public-keys = [
    "bytecodealliance.cachix.org-1:0SBgh//n2n0heh0sDFhTm+ZKBRy2sInakzFGfzN531Y="
    "nixify.cachix.org-1:95SiUQuf8Ij0hwDweALJsLtnMyv/otZamWNRp1Q1pXw="
    "crane.cachix.org-1:8Scfpmn9w+hGdXH/Q9tTLiYAE/2dnJYRJP7kl80GuRk="
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  ];

  inputs.nixify.inputs.nixlib.follows = "nixlib";
  inputs.nixify.url = "github:rvolosatovs/nixify";
  inputs.nixlib.url = "github:nix-community/nixpkgs.lib";

  outputs =
    {
      nixify,
      nixlib,
      ...
    }:
    with nixlib.lib;
    with nixify.lib;
    rust.mkFlake {
      src = ./.;

      excludePaths = [
        ".envrc"
        ".github"
        ".gitignore"
        "flake.nix"
        "LICENSE"
        "README.md"
      ];

      doCheck = false; # testing is performed in checks via `nextest`

      clippy.allTargets = true;
      clippy.deny = [ "warnings" ];
      clippy.workspace = true;

      test.allTargets = true;
      test.workspace = true;
    };
}
