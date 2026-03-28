#!/bin/bash
# Dev helper script for braille text browser
set -e

case "${1:-build}" in
  build)
    cargo build --bin braille-engine --bin braille
    # Kill stale daemon so next command uses the fresh binary
    pkill -f 'braille.* daemon start' 2>/dev/null || true
    ;;
  test)
    cargo test -p braille-engine --test adversarial
    cargo test -p braille-engine --test engine_repl
    cargo test -p braille-engine --test webpack_chunks
    cargo test -p braille-engine --test link_onload
    cargo test -p braille-engine --test link_basic
    cargo test -p braille-cli --test kitchen_sink
    cargo test -p braille-cli --test spa_dynamic
    cargo test -p braille-cli --test webpack_spa
    ;;
  check)
    cargo clippy --workspace --exclude spike-quickjs
    ;;
  proton)
    cargo run --bin braille -- daemon stop 2>/dev/null || true
    sleep 1
    SES=$(cargo run --bin braille -- new 2>&1 | tail -1)
    echo "Session: $SES"
    cargo run --bin braille -- "$SES" goto "https://account.proton.me/login"
    ;;
  *)
    echo "Usage: ./dev.sh [build|test|check|proton]"
    ;;
esac
