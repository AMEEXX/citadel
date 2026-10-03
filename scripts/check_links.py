import os
import re

def check_file_links(file_path):
    print(f"Checking links in {file_path}...")
    base_dir = os.path.dirname(os.path.abspath(file_path))
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Match [text](link) where link doesn't start with http, mailto, or #
    pattern = r'\[([^\]]+)\]\((?!http|mailto|#)([^\)]+)\)'
    matches = re.findall(pattern, content)
    
    missing = []
    found = 0
    for text, link in matches:
        # Strip anchor if present
        clean_link = link.split('#')[0]
        if not clean_link:
            continue
        target_path = os.path.normpath(os.path.join(base_dir, clean_link))
        if os.path.exists(target_path):
            found += 1
        else:
            missing.append((text, link, target_path))
            
    print(f"  Valid links: {found}")
    if missing:
        print(f"  MISSING LINKS ({len(missing)}):")
        for text, link, target_path in missing:
            print(f"    - [{text}]({link}) -> {target_path}")
    else:
        print("  All local links are 100% valid!")

check_file_links("README.md")
check_file_links("docs/README.md")
check_file_links("bin/README.md")