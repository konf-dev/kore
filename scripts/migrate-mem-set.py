#!/usr/bin/env python3
"""
Migrate Kore files from old mem-set/rom-set syntax to new syntax.

Old: "key" value mem-set
New: value "key" mem-set

This follows the Kore principle: value first, then name (like def).
"""

import re
import sys

def migrate_file(filename):
    with open(filename) as f:
        content = f.read()
    
    lines = content.split('\n')
    new_lines = []
    
    for line in lines:
        # Skip comments
        if line.strip().startswith('#'):
            new_lines.append(line)
            continue
        
        # Pattern 1: "key" literal mem-set -> literal "key" mem-set
        # Match: "key" followed by simple value (number, true, false, null, list-empty)
        line = re.sub(
            r'"(\w[\w-]*)" ((?:-?\d+(?:\.\d+)?|true|false|null|list-empty)) mem-set',
            r'\2 "\1" mem-set',
            line
        )
        
        # Pattern 2: "key" swap mem-set -> swap "key" mem-set
        line = re.sub(
            r'"(\w[\w-]*)" swap mem-set',
            r'swap "\1" mem-set',
            line
        )
        
        # Pattern 3: "key" "key" mem-get expr mem-set -> "key" mem-get expr "key" mem-set
        # This is tricky - need to handle the increment pattern
        # "x" "x" mem-get 1 add mem-set -> "x" mem-get 1 add "x" mem-set
        line = re.sub(
            r'"(\w[\w-]*)" "\1" mem-get (.+?) mem-set',
            r'"\1" mem-get \2 "\1" mem-set',
            line
        )
        
        # Same patterns for rom-set
        line = re.sub(
            r'"(\w[\w-]*)" ((?:-?\d+(?:\.\d+)?|true|false|null|list-empty)) rom-set',
            r'\2 "\1" rom-set',
            line
        )
        line = re.sub(
            r'"(\w[\w-]*)" swap rom-set',
            r'swap "\1" rom-set',
            line
        )
        line = re.sub(
            r'"(\w[\w-]*)" "\1" mem-get (.+?) rom-set',
            r'"\1" mem-get \2 "\1" rom-set',
            line
        )
        
        new_lines.append(line)
    
    return '\n'.join(new_lines)

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: migrate-mem-set.py <file.kore> [--inplace]")
        sys.exit(1)
    
    filename = sys.argv[1]
    inplace = '--inplace' in sys.argv
    
    result = migrate_file(filename)
    
    if inplace:
        with open(filename, 'w') as f:
            f.write(result)
        print(f"Migrated {filename} in place")
    else:
        print(result)
