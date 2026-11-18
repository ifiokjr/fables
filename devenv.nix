{ pkgs, lib, ... }:

{
  # The shell's stdenv brings a Nix clang and Apple SDK, so builds work without accepting the
  # Xcode licence. Rust itself comes from rustup, like the rest of the workspace.
  packages =
    with pkgs;
    [
      epubcheck
      ffmpeg
      poppler-utils
      rustup
    ]
    ++ lib.optionals stdenv.isDarwin [
      libiconv
    ];

  scripts = {
    "fables".exec = ''cargo run --quiet --release -- "$@"'';
    "check:all".exec = ''
      set -e
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      cargo test
    '';
    "check:worlds".exec = ''./scripts/check-worlds.sh'';
    "check:epub".exec = ''epubcheck "$@"'';
  };

  cachix.enable = false;
  dotenv.disableHint = true;
}
