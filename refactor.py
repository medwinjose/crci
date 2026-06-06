import os
import shutil

# 1. Setup crci-core directories
os.makedirs("crci-core/src/transport", exist_ok=True)
os.makedirs("crci-core/src/storage", exist_ok=True)

# 2. Extract dependencies from Cargo.toml
with open("Cargo.toml", "r") as f:
    cargo_toml = f.read()

deps_section = cargo_toml[cargo_toml.find("[dependencies]"):]

# 3. Create crci-core/Cargo.toml
core_cargo = f"""[package]
name = "crci-core"
version = "0.1.0"
edition = "2021"

[lib]
name = "crci_core"
path = "src/lib.rs"

{deps_section}
"""
with open("crci-core/Cargo.toml", "w") as f:
    f.write(core_cargo)

# 4. Modify root Cargo.toml
new_root_cargo = f"""[workspace]
members = [".", "crci-core"]
resolver = "2"

[package]
name = "crci"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "crci"
path = "src/main.rs"

{deps_section}
"""
# Add crci-core dependency
new_root_cargo = new_root_cargo.replace("[dependencies]", '[dependencies]\ncrci-core = { path = "crci-core" }')

with open("Cargo.toml", "w") as f:
    f.write(new_root_cargo)

# 5. Move files
files_to_move = [f for f in os.listdir("src") if f.endswith(".rs") and f not in ["main.rs", "cli.rs"]]
for f in files_to_move:
    shutil.move(f"src/{f}", f"crci-core/src/{f}")

# Move directories
for d in ["storage", "transport"]:
    if os.path.exists(f"src/{d}"):
        for f in os.listdir(f"src/{d}"):
            shutil.move(f"src/{d}/{f}", f"crci-core/src/{d}/{f}")
        os.rmdir(f"src/{d}")

# Note: src/lib.rs was moved to crci-core/src/lib.rs.
# It already has the correct `pub mod` declarations because we updated it earlier.

# 6. Update src/main.rs
with open("src/main.rs", "r") as f:
    main_rs = f.read()

# Remove module declarations
import re
# Remove `mod xxx;` and `pub mod xxx;` EXCEPT `mod cli;`
lines = main_rs.split('\n')
new_lines = []
for line in lines:
    stripped = line.strip()
    if (stripped.startswith("mod ") or stripped.startswith("pub mod ")) and stripped.endswith(";"):
        if "cli" in stripped:
            new_lines.append(line)
    else:
        new_lines.append(line)

main_rs = '\n'.join(new_lines)
# Add `use crci_core::*;` or just rely on existing imports if any.
# Wait, `main.rs` uses `crate::api::ApiState`, etc.
# We must replace `crate::` with `crci_core::` in main.rs and bin/*.rs!
main_rs = main_rs.replace("crate::", "crci_core::")
with open("src/main.rs", "w") as f:
    f.write(main_rs)

# 7. Update bin/*.rs
if os.path.exists("src/bin"):
    for f in os.listdir("src/bin"):
        if f.endswith(".rs"):
            path = f"src/bin/{f}"
            with open(path, "r") as file:
                content = file.read()
            content = content.replace("crci::", "crci_core::")
            content = content.replace("crate::", "crci_core::")
            with open(path, "w") as file:
                file.write(content)

# 8. Update tests/
if os.path.exists("tests"):
    for f in os.listdir("tests"):
        if f.endswith(".rs"):
            path = f"tests/{f}"
            with open(path, "r") as file:
                content = file.read()
            content = content.replace("crci::", "crci_core::")
            with open(path, "w") as file:
                file.write(content)
