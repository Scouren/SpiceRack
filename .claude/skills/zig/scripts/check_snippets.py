#!/usr/bin/env python3
"""Compile every ```zig block in this skill's .md files under `zig test`.

Usage: check_snippets.py [zig-binary] [--keep DIR]

Each block becomes its own file: a prelude (std, assert, Allocator) plus the block, plus a test
that references every top-level non-generic fn so lazy analysis cannot skip it. A block whose
first line is `// build.zig` or `// not compiled:` is skipped; verify those in a real project.
Exit status 1 on the first failing block, with the compiler output.
"""

import pathlib, re, subprocess, sys, tempfile

PRELUDE = 'const std = @import("std");\nconst assert = std.debug.assert;\nconst Allocator = std.mem.Allocator;\n'
FN = re.compile(r"^(?:pub )?(?:export )?(?:inline )?fn (\w+)\(([^)]*)\)", re.M)

def main(argv):
    zig = next((a for a in argv[1:] if not a.startswith("--")), "zig")
    keep = argv[argv.index("--keep") + 1] if "--keep" in argv else None
    out = pathlib.Path(keep or tempfile.mkdtemp())
    out.mkdir(parents=True, exist_ok=True)
    root = pathlib.Path(__file__).resolve().parent.parent
    count = 0
    for md in sorted(root.glob("*.md")):
        for i, block in enumerate(re.findall(r"^```zig\n(.*?)^```", md.read_text(), re.S | re.M)):
            if block.startswith(("// build.zig", "// not compiled:")):
                continue
            refs = [n for n, params in FN.findall(block) if "comptime" not in params and "anytype" not in params]
            src = PRELUDE + block + "test {\n" + "".join(f"    _ = &{n};\n" for n in refs) + "}\n"
            path = out / f"{md.stem}_{i}.zig"
            path.write_text(src)
            r = subprocess.run([zig, "test", str(path)], capture_output=True, text=True)
            if r.returncode != 0:
                print(f"FAIL {md.name} block {i} ({path})\n{r.stderr}")
                return 1
            count += 1
    print(f"check_snippets: {count} blocks compiled and passed")
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
