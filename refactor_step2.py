import os

# 6. Update src/main.rs
with open("src/main.rs", "r", encoding="utf-8") as f:
    main_rs = f.read()

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

# Replace crate:: with crci_core::
main_rs = main_rs.replace("crate::", "crci_core::")
# also need to import `use crci_core::*;` or let them use it via crci_core::api etc.
# Actually `main.rs` doesn't just use `crate::api`, it sometimes uses `crate::api::ApiState`. That's handled by replace.
# What if it uses `api::ApiState` directly without `crate::`? Wait, `mod api` makes it `api::`. So we might need `use crci_core::*;` at the top!
new_lines.insert(0, "use crci_core::*;")

main_rs = '\n'.join(new_lines)
main_rs = main_rs.replace("crate::", "crci_core::")
with open("src/main.rs", "w", encoding="utf-8") as f:
    f.write(main_rs)

# 7. Update bin/*.rs
if os.path.exists("src/bin"):
    for f in os.listdir("src/bin"):
        if f.endswith(".rs"):
            path = f"src/bin/{f}"
            with open(path, "r", encoding="utf-8") as file:
                content = file.read()
            content = content.replace("crci::", "crci_core::")
            content = content.replace("crate::", "crci_core::")
            with open(path, "w", encoding="utf-8") as file:
                file.write(content)

# 8. Update tests/
if os.path.exists("tests"):
    for f in os.listdir("tests"):
        if f.endswith(".rs"):
            path = f"tests/{f}"
            with open(path, "r", encoding="utf-8") as file:
                content = file.read()
            content = content.replace("crci::", "crci_core::")
            with open(path, "w", encoding="utf-8") as file:
                file.write(content)
