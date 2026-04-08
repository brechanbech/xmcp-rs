#!/bin/sh
# Download or update the Xojo documentation files used by xmcp's doc tools
# (search_docs, lookup_class, list_doc_topics).
#
# Usage:
#   scripts/update-xojo-docs.sh            # auto-detect path
#   scripts/update-xojo-docs.sh /path/to   # explicit target directory

set -e

BASE_URL="https://docs.xojo.com"

if [ -n "$1" ]; then
    docs_dir="$1"
else
    # Auto-detect: pick the newest Xojo version directory that already has
    # Documentation, or the newest version directory if none do.
    xojo_base="$HOME/Library/Application Support/Xojo/Xojo"
    if [ ! -d "$xojo_base" ]; then
        echo "Error: $xojo_base does not exist. Is Xojo installed?" >&2
        exit 1
    fi

    # Find the newest version directory (by name, descending).
    newest=""
    for d in "$xojo_base"/*/; do
        name=$(basename "$d")
        # Skip non-version dirs like "Filters", "License Keys".
        case "$name" in
            Xojo*|[0-9]*) ;;
            *) continue ;;
        esac
        if [ -z "$newest" ] || [ "$name" \> "$newest" ]; then
            newest="$name"
        fi
    done

    if [ -z "$newest" ]; then
        echo "Error: no Xojo version directories found in $xojo_base" >&2
        exit 1
    fi

    docs_dir="$xojo_base/$newest/Documentation"
fi

mkdir -p "$docs_dir"

echo "Downloading Xojo documentation to: $docs_dir"

echo "  llms.txt ..."
curl -fSL -o "$docs_dir/llms.txt" "$BASE_URL/llms.txt"

echo "  llms-full.txt (~15 MB) ..."
curl -fSL -o "$docs_dir/llms-full.txt" "$BASE_URL/llms-full.txt"

# ---------------------------------------------------------------------------
# Split llms-full.txt into individual _sources/*.rst.txt files so that
# lookup_class can do fast, targeted reads instead of scanning the full file.
#
# The RST structure uses a type marker (e.g. "Class", "DataType") on its own
# line, followed by a blank line, then an === underline, the name, and another
# === underline.  We split on that pattern and name each file after the class.
# ---------------------------------------------------------------------------
sources_dir="$docs_dir/_sources"
rm -rf "$sources_dir"
mkdir -p "$sources_dir"

echo "  Splitting into _sources/*.rst.txt ..."

awk '
    # Split on the pattern:
    #   <type marker>    (e.g. "Class" or "DataType" alone on a line)
    #   <blank line>
    #   ====...====
    #   ClassName
    #   ====...====
    #   <content ...>
    #
    # State machine: marker -> skip_blank -> open_underline -> read_name -> close_underline
    BEGIN { state = "scan"; file = ""; count = 0 }

    state == "scan" && /^(Class|DataType)$/ {
        marker = $0
        state = "skip_blank"
        next
    }

    state == "skip_blank" && /^$/ {
        state = "open_underline"
        next
    }

    state == "open_underline" && /^=+$/ {
        underline = $0
        state = "read_name"
        next
    }

    state == "read_name" {
        name = $0
        state = "close_underline"
        next
    }

    state == "close_underline" && /^=+$/ {
        # Start a new file.
        lname = tolower(name)
        gsub(/ /, "_", lname)
        file = sources "/" lname ".rst.txt"
        count++

        print marker > file
        print "" > file
        print underline > file
        print name > file
        print $0 > file

        state = "scan"
        marker = ""
        next
    }

    # Pattern did not complete — reset and treat as content.
    state != "scan" {
        state = "scan"
        marker = ""
    }

    # Normal content — append to current file.
    file != "" { print > file }

    END {
        printf "  %d class files written to _sources/\n", count
    }
' sources="$sources_dir" "$docs_dir/llms-full.txt"

echo "Done. Documentation updated at: $docs_dir"
