#!/bin/bash

# Script to auto-update README.md with docs and project structure

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
README="$REPO_ROOT/README.md"
DOCS_DIR="$REPO_ROOT/docs"
SRC_DIR="$REPO_ROOT/src"

# Extract the first heading from a markdown file as its title
get_title() {
    local file="$1"
    title=$(grep -m 1 '^#' "$file" 2>/dev/null | sed 's/^#* *//')
    if [ -z "$title" ]; then
        title=$(basename "$file" .md)
    fi
    echo "$title"
}

# Extract first line comment from .rs file as description
get_rs_description() {
    local file="$1"
    # Look for //! or // comment at start of file
    desc=$(head -5 "$file" | grep -m 1 '^//[!/]* *' | sed 's|^//[!/]* *||')
    if [ -z "$desc" ]; then
        echo ""
    else
        echo "# $desc"
    fi
}

# Generate project structure
generate_structure() {
    echo "## Project Structure"
    echo ""
    echo '```'
    echo "src/"
    
    # List src files with tree-like format
    local files=("$SRC_DIR"/*.rs)
    local count=${#files[@]}
    local i=0
    
    for file in "${files[@]}"; do
        if [ -f "$file" ]; then
            i=$((i + 1))
            filename=$(basename "$file")
            desc=$(get_rs_description "$file")
            
            if [ $i -eq $count ]; then
                prefix="└──"
            else
                prefix="├──"
            fi
            
            if [ -n "$desc" ]; then
                printf "%s %-20s %s\n" "$prefix" "$filename" "$desc"
            else
                echo "$prefix $filename"
            fi
        fi
    done
    
    echo ""
    echo "docs/"
    
    # List doc files
    local docs=("$DOCS_DIR"/*.md)
    local dcount=${#docs[@]}
    local j=0
    
    for file in "${docs[@]}"; do
        if [ -f "$file" ]; then
            j=$((j + 1))
            filename=$(basename "$file")
            
            if [ $j -eq $dcount ]; then
                prefix="└──"
            else
                prefix="├──"
            fi
            
            echo "$prefix $filename"
        fi
    done
    
    echo '```'
}

# Generate documentation links
generate_doc_links() {
    echo "## Documentation"
    echo ""
    for file in "$DOCS_DIR"/*.md; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            title=$(get_title "$file")
            echo "- [$title](docs/$filename)"
        fi
    done
}

# Read README and replace sections
update_readme() {
    local temp_file=$(mktemp)
    local current_section=""
    local structure_written=false
    local docs_written=false
    
    while IFS= read -r line || [ -n "$line" ]; do
        # Detect section starts
        if [[ "$line" == "## Project Structure"* ]]; then
            current_section="structure"
            if [ "$structure_written" = false ]; then
                generate_structure >> "$temp_file"
                structure_written=true
            fi
            continue
        elif [[ "$line" == "## Documentation"* ]]; then
            current_section="docs"
            if [ "$docs_written" = false ]; then
                generate_doc_links >> "$temp_file"
                docs_written=true
            fi
            continue
        elif [[ "$line" == "## "* ]]; then
            # New section, end previous
            if [ -n "$current_section" ]; then
                echo "" >> "$temp_file"
            fi
            current_section=""
        fi
        
        # Skip lines in auto-generated sections
        if [ "$current_section" = "structure" ] || [ "$current_section" = "docs" ]; then
            continue
        fi
        
        echo "$line" >> "$temp_file"
    done < "$README"
    
    mv "$temp_file" "$README"
    echo "README.md updated: $(ls -1 "$SRC_DIR"/*.rs 2>/dev/null | wc -l | tr -d ' ') src files, $(ls -1 "$DOCS_DIR"/*.md 2>/dev/null | wc -l | tr -d ' ') docs"
}

update_readme
