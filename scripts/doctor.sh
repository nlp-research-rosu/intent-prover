#!/bin/sh

plugin_root=$1
plugin_data=$2

# The staging catalog installs raw staging source under this data directory;
# versioned staging packages instead carry their channel in release/channel.
case "$plugin_data" in
    *[/\\]intent-intentcomputing-staging) cli=intent-prover-staging ;;
    *)
        if [ "$(cat "$plugin_root/release/channel" 2>/dev/null)" = staging ]; then
            cli=intent-prover-staging
        else
            cli=intent-prover
        fi
        ;;
esac

# Claude's command expansion needs a successful shell exit to display diagnostics.
"$cli" doctor --data "$plugin_data" --plugin-root "$plugin_root" 2>&1 || true
